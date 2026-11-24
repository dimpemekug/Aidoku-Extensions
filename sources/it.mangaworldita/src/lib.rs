#![no_std]
use aidoku::{
	BaseUrlProvider, Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent,
	HomeComponentValue, HomeLayout, ImageRequestProvider, Link, Listing, ListingKind,
	ListingProvider, Manga, MangaPageResult, Page, PageContext, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{
		defaults::defaults_get,
		html::Document,
		net::{Request, TimeUnit, set_rate_limit},
	},
	prelude::*,
};

pub mod parse;

use parse::{
	DEFAULT_DOMAIN, chapter_id_from_url, chapter_url, manga_id_from_url, manga_url,
	normalized_domain, parse_archive, parse_chapters, parse_details, parse_latest, parse_pages,
	parse_trending,
};

/// The sort options of the archive, in the order of `filters.json`.
const SORTS: [&str; 7] = ["most_read", "less_read", "top_rated", "newest", "oldest", "a-z", "z-a"];

/// The address MangaWorld uses since November 2026, unless the settings say otherwise.
fn base_url() -> String {
	defaults_get::<String>("domain")
		.map(|value| normalized_domain(&value))
		.filter(|value| !value.is_empty())
		.unwrap_or_else(|| DEFAULT_DOMAIN.into())
}

fn get(url: &str) -> Result<Document> {
	let base = base_url();
	Ok(Request::get(url)?.header("Referer", &format!("{base}/")).html()?)
}

/// The archive URL for a search, a sorted list or a genre.
fn archive_url(base: &str, page: i32, query: Option<&str>, filters: &[FilterValue]) -> String {
	let mut qs = QueryParameters::new();
	if let Some(keyword) = query.map(str::trim).filter(|q| !q.is_empty()) {
		qs.push("keyword", Some(keyword));
	}
	for filter in filters {
		match filter {
			FilterValue::Sort { id, index, .. } => {
				// "Pertinenza" only means something with a keyword: the site picks it then.
				if query.is_none_or(|q| q.trim().is_empty()) {
					if let Some(sort) = SORTS.get(*index as usize) {
						qs.set(id, Some(sort));
					}
				}
			}
			FilterValue::MultiSelect { id, included, excluded } => {
				for value in included {
					qs.push(id, Some(value));
				}
				if id == "genre" {
					for value in excluded {
						qs.push("exclude", Some(value));
					}
				}
			}
			FilterValue::Text { id, value } if !value.trim().is_empty() => {
				qs.set(id, Some(value.trim()));
			}
			FilterValue::Select { id, value } => {
				qs.set(id, Some(value));
			}
			_ => {}
		}
	}
	if page > 1 {
		qs.set("page", Some(&page.to_string()));
	}
	if qs.is_empty() { format!("{base}/archive") } else { format!("{base}/archive?{qs}") }
}

fn sorted_archive(base: &str, sort: &str, page: i32) -> String {
	let index = SORTS.iter().position(|s| *s == sort).unwrap_or(0) as i32;
	archive_url(base, page, None, &[FilterValue::Sort { id: "sort".into(), index, ascending: false }])
}

fn listing(id: &str, name: &str) -> Listing {
	Listing { id: id.into(), name: name.into(), kind: ListingKind::Default }
}

struct MangaWorldITA;

impl Source for MangaWorldITA {
	fn new() -> Self {
		set_rate_limit(5, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = archive_url(&base_url(), page, query.as_deref(), &filters);
		Ok(parse_archive(&get(&url)?))
	}

	fn get_manga_update(&self, mut manga: Manga, needs_details: bool, needs_chapters: bool) -> Result<Manga> {
		let base = base_url();
		let html = get(&manga_url(&base, &manga.key))?;
		if needs_details {
			manga = parse_details(&html, &base, manga);
		}
		if needs_chapters {
			manga.chapters = Some(parse_chapters(&html, &base, &manga.key));
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let pages = parse_pages(&get(&chapter_url(&base_url(), &manga.key, &chapter.key))?);
		if pages.is_empty() {
			bail!("Nessuna pagina trovata in questo capitolo");
		}
		Ok(pages)
	}
}

impl ListingProvider for MangaWorldITA {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let base = base_url();
		match listing.id.as_str() {
			"latest" => {
				let url = if page > 1 { format!("{base}/?page={page}") } else { base.clone() };
				let (entries, has_next_page) = parse_latest(&get(&url)?);
				Ok(MangaPageResult { entries: entries.into_iter().map(|e| e.manga).collect(), has_next_page })
			}
			sort => Ok(parse_archive(&get(&sorted_archive(&base, sort, page))?)),
		}
	}
}

impl Home for MangaWorldITA {
	fn get_home(&self) -> Result<HomeLayout> {
		let base = base_url();
		let home = get(&base)?;
		let (latest, _) = parse_latest(&home);
		let mut components = vec![
			HomeComponent {
				title: Some("Capitoli di tendenza".into()),
				subtitle: None,
				value: HomeComponentValue::BigScroller { entries: parse_trending(&home), auto_scroll_interval: Some(8.0) },
			},
			HomeComponent {
				title: Some("Ultimi capitoli aggiunti".into()),
				subtitle: None,
				value: HomeComponentValue::MangaChapterList {
					page_size: Some(8),
					entries: latest,
					listing: Some(listing("latest", "Ultimi capitoli")),
				},
			},
		];
		for (id, title, ranking) in
			[("most_read", "Più letti", true), ("newest", "Ultime aggiunte", false), ("top_rated", "Più votati", false)]
		{
			let entries: Vec<Link> = parse_archive(&get(&sorted_archive(&base, id, 1))?)
				.entries
				.into_iter()
				.map(Link::from)
				.collect();
			let listing = Some(listing(id, title));
			components.push(HomeComponent {
				title: Some(title.into()),
				subtitle: None,
				value: if ranking {
					HomeComponentValue::MangaList { ranking: true, page_size: Some(5), entries, listing }
				} else {
					HomeComponentValue::Scroller { entries, listing }
				},
			});
		}
		Ok(HomeLayout { components })
	}
}

impl ImageRequestProvider for MangaWorldITA {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		let base = base_url();
		Ok(Request::get(url)?.header("Referer", &format!("{base}/")))
	}
}

impl BaseUrlProvider for MangaWorldITA {
	fn get_base_url(&self) -> Result<String> {
		Ok(base_url())
	}
}

impl DeepLinkHandler for MangaWorldITA {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(manga_key) = manga_id_from_url(&url) else {
			return Ok(None);
		};
		Ok(Some(match chapter_id_from_url(&url) {
			Some(key) => DeepLinkResult::Chapter { manga_key, key },
			None => DeepLinkResult::Manga { key: manga_key },
		}))
	}
}

register_source!(MangaWorldITA, ListingProvider, Home, ImageRequestProvider, BaseUrlProvider, DeepLinkHandler);

#[cfg(test)]
mod tests;
