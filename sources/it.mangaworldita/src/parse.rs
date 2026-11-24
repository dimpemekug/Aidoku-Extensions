// Reads the HTML of www.mangaworld.me (the theme introduced in November 2026).
// Functions over a parsed page and the site's base URL only: no requests here,
// so the tests can feed them saved pages.

use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, MangaWithChapter, Page,
	PageContent, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::decode_uri,
	imports::html::{Document, Element},
	prelude::*,
};

pub const DEFAULT_DOMAIN: &str = "https://www.mangaworld.me";

/// "www.example.com/" → "https://www.example.com": the form the setting expects.
pub fn normalized_domain(value: &str) -> String {
	let trimmed = value.trim().trim_end_matches('/');
	if trimmed.is_empty() {
		return String::new();
	}
	if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
		trimmed.into()
	} else {
		let mut domain = String::from("https://");
		domain.push_str(trimmed);
		domain
	}
}

/// The numeric id in `/manga/<id>/<slug>`: the site ignores the slug.
pub fn manga_id_from_url(url: &str) -> Option<String> {
	let rest = &url[url.find("/manga/")? + "/manga/".len()..];
	let id: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
	if id.is_empty() { None } else { Some(id) }
}

/// The id after `/read/` in a chapter link.
pub fn chapter_id_from_url(url: &str) -> Option<String> {
	let rest = &url[url.find("/read/")? + "/read/".len()..];
	let id: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
	if id.is_empty() { None } else { Some(id) }
}

pub fn manga_url(base: &str, manga_id: &str) -> String {
	format!("{base}/manga/{manga_id}")
}

/// Any slug works: the site answers with the right chapter.
pub fn chapter_url(base: &str, manga_id: &str, chapter_id: &str) -> String {
	format!("{base}/manga/{manga_id}/_/read/{chapter_id}")
}

/// One query parameter of a link, decoded.
pub fn query_param(href: &str, name: &str) -> Option<String> {
	let query = href.split('#').next()?.split_once('?')?.1;
	for pair in query.split('&') {
		let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
		if key == name {
			return Some(decode_uri(value.replace('+', " ")));
		}
	}
	None
}

/// Drops the cache-busting `?123` so the same cover keeps the same address.
pub fn clean_image(src: &str) -> String {
	match src.rsplit_once('?') {
		Some((path, query)) if !query.is_empty() && query.chars().all(|c| c.is_ascii_digit()) => path.into(),
		_ => src.into(),
	}
}

const ADULT_GENRES: [&str; 7] = ["adulti", "hentai", "smut", "yaoi", "yuri", "lolicon", "shotacon"];
const MATURE_GENRES: [&str; 2] = ["ecchi", "maturo"];

pub fn rating_for_genres<S: AsRef<str>>(genres: &[S]) -> ContentRating {
	if genres.iter().any(|g| ADULT_GENRES.contains(&g.as_ref())) {
		ContentRating::NSFW
	} else if genres.iter().any(|g| MATURE_GENRES.contains(&g.as_ref())) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

fn text(element: Option<Element>) -> Option<String> {
	element.and_then(|el| el.text()).filter(|t| !t.is_empty())
}

fn genre_ids(scope: &Element) -> Vec<String> {
	scope
		.select("a[href*=\"genre=\"]")
		.map(|links| {
			links
				.filter_map(|a| query_param(&a.attr("href")?, "genre"))
				.filter(|g| !g.is_empty())
				.collect()
		})
		.unwrap_or_default()
}

fn page_number(href: &str) -> i32 {
	query_param(href, "page").and_then(|p| p.parse().ok()).unwrap_or(0)
}

/// The pager under every list: the current page is marked, the others are `page=` links.
pub fn has_next_page(html: &Document) -> bool {
	let current = text(html.select_first("[aria-current=page][aria-label^=Pagina]"))
		.and_then(|t| t.parse::<i32>().ok())
		.unwrap_or(1);
	let last = html
		.select("a[href*=\"page=\"]")
		.map(|links| links.filter_map(|a| a.attr("href")).map(|h| page_number(&h)).max().unwrap_or(0))
		.unwrap_or(0);
	current < last
}

// MARK: Lists

/// The archive (`/archive?…`): search results and every sorted list.
pub fn parse_archive(html: &Document) -> MangaPageResult {
	let entries: Vec<Manga> = html
		.select("article.archive-card")
		.map(|cards| {
			cards
				.filter_map(|card| {
					let link = card.select_first(".card-title a")?;
					let key = manga_id_from_url(&link.attr("href")?)?;
					let genres = genre_ids(&card);
					let mut tags = Vec::new();
					if let Some(kind) = text(card.select_first(".card-tags .tag")) {
						tags.push(kind);
					}
					Some(Manga {
						key,
						title: link.text()?,
						cover: card.select_first(".card-cover img").and_then(|img| img.attr("src")).map(|s| clean_image(&s)),
						description: text(card.select_first(".archive-plot")),
						content_rating: rating_for_genres(&genres),
						tags: if tags.is_empty() { None } else { Some(tags) },
						..Default::default()
					})
				})
				.collect()
		})
		.unwrap_or_default();
	let has_next_page = !entries.is_empty() && has_next_page(html);
	MangaPageResult { entries, has_next_page }
}

/// "Capitoli di tendenza", the carousel at the top of the home page: each series once.
pub fn parse_trending(html: &Document) -> Vec<Manga> {
	let mut seen: Vec<String> = Vec::new();
	html.select("#hero article.slide")
		.map(|slides| {
			slides
				.filter_map(|slide| {
					let link = slide.select_first(".slide-title a")?;
					let key = manga_id_from_url(&link.attr("href")?)?;
					if seen.contains(&key) {
						return None;
					}
					seen.push(key.clone());
					Some(Manga {
						key,
						title: link.text()?,
						cover: slide.select_first(".slide-cover img").and_then(|img| img.attr("src")).map(|s| clean_image(&s)),
						description: text(slide.select_first(".slide-plot")),
						content_rating: rating_for_genres(&genre_ids(&slide)),
						..Default::default()
					})
				})
				.collect()
		})
		.unwrap_or_default()
}

/// "Ultimi capitoli aggiunti": the newest chapter of each series, paged with `?page=`.
pub fn parse_latest(html: &Document) -> (Vec<MangaWithChapter>, bool) {
	let entries: Vec<MangaWithChapter> = html
		.select("#latest-grid article.card")
		.map(|cards| {
			cards
				.filter_map(|card| {
					let title = card.select_first(".card-title a")?;
					let key = manga_id_from_url(&title.attr("href")?)?;
					let newest = card.select_first(".ch-list a.ch")?;
					let chapter_key = chapter_id_from_url(&newest.attr("href")?)?;
					let label = newest.attr("title").unwrap_or_default();
					let (chapter_number, _) = parse_chapter_name(label.rsplit(" - ").next().unwrap_or(&label));
					Some(MangaWithChapter {
						manga: Manga {
							key,
							title: title.text()?,
							cover: card.select_first(".card-cover img").and_then(|img| img.attr("src")).map(|s| clean_image(&s)),
							..Default::default()
						},
						chapter: Chapter {
							key: chapter_key,
							chapter_number,
							title: if label.is_empty() { None } else { Some(label) },
							date_uploaded: newest
								.select_first("time")
								.and_then(|t| t.attr("datetime"))
								.and_then(|d| parse_iso_date(&d)),
							language: Some("it".into()),
							..Default::default()
						},
					})
				})
				.collect()
		})
		.unwrap_or_default();
	let has_next = !entries.is_empty() && has_next_page(html);
	(entries, has_next)
}

// MARK: Series

/// Fills the details of `manga` from its page.
pub fn parse_details(html: &Document, base: &str, mut manga: Manga) -> Manga {
	if let Some(title) = text(html.select_first("h1.manga-title")) {
		manga.title = title;
	}
	manga.cover = html
		.select_first(".manga-cover img")
		.and_then(|img| img.attr("src"))
		.map(|s| clean_image(&s))
		.or(manga.cover);
	manga.description = text(html.select_first(".manga-plot"));
	manga.authors = text(html.select_first(".manga-facts a[href*=\"author=\"]")).map(|a| vec![a]);
	manga.artists = text(html.select_first(".manga-facts a[href*=\"artist=\"]")).map(|a| vec![a]);
	manga.url = Some(manga_url(base, &manga.key));

	let chips: Vec<(String, String)> = html
		.select(".manga-info .hero-chips a")
		.map(|links| {
			links
				.filter_map(|a| Some((query_param(&a.attr("href")?, "genre")?, a.text()?)))
				.collect()
		})
		.unwrap_or_default();
	let ids: Vec<&String> = chips.iter().map(|(id, _)| id).collect();
	manga.content_rating = rating_for_genres(&ids);
	manga.tags = if chips.is_empty() { None } else { Some(chips.iter().map(|(_, t)| t.clone()).collect()) };

	let status = html
		.select_first(".manga-facts a[href*=\"status=\"]")
		.and_then(|a| a.attr("href"))
		.and_then(|h| query_param(&h, "status"))
		.unwrap_or_default();
	manga.status = match status.as_str() {
		"ongoing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"paused" => MangaStatus::Hiatus,
		"dropped" | "canceled" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	};

	let kind = html
		.select_first(".manga-facts a[href*=\"type=\"]")
		.and_then(|a| a.attr("href"))
		.and_then(|h| query_param(&h, "type"))
		.unwrap_or_default();
	manga.viewer = match kind.as_str() {
		"manhwa" | "manhua" => Viewer::Webtoon,
		"manga" | "oneshot" | "doujinshi" => Viewer::RightToLeft,
		_ => Viewer::Unknown,
	};
	manga
}

/// Every chapter, newest first, the way the series page lists them.
pub fn parse_chapters(html: &Document, base: &str, manga_id: &str) -> Vec<Chapter> {
	let mut chapters = Vec::new();
	let volumes = html.select("#panel-capitoli .volume").map(|v| v.collect::<Vec<_>>()).unwrap_or_default();
	if volumes.is_empty() {
		if let Some(rows) = html.select("#panel-capitoli li.ch-row") {
			for row in rows {
				chapters.extend(parse_chapter_row(&row, None, base, manga_id));
			}
		}
	} else {
		for volume in volumes {
			let number = text(volume.select_first(".volume-name")).and_then(|name| first_number(&name));
			if let Some(rows) = volume.select("li.ch-row") {
				for row in rows {
					chapters.extend(parse_chapter_row(&row, number, base, manga_id));
				}
			}
		}
	}
	chapters
}

fn parse_chapter_row(row: &Element, volume: Option<f32>, base: &str, manga_id: &str) -> Option<Chapter> {
	let link = row.select_first("a.ch")?;
	let key = row.attr("data-chapter").or_else(|| chapter_id_from_url(&link.attr("href")?))?;
	let name = text(link.select_first(".ch-name")).unwrap_or_default();
	let (chapter_number, title) = parse_chapter_name(&name);
	Some(Chapter {
		url: Some(chapter_url(base, manga_id, &key)),
		key,
		title,
		chapter_number,
		volume_number: volume,
		date_uploaded: text(link.select_first(".ch-date")).and_then(|d| parse_italian_date(&d)),
		language: Some("it".into()),
		..Default::default()
	})
}

/// The reader page lists every page as an `<img>`.
pub fn parse_pages(html: &Document) -> Vec<Page> {
	html.select(".reader-stage img.reader-page")
		.map(|images| {
			images
				.filter_map(|img| img.attr("src").or_else(|| img.attr("data-src")))
				.filter(|src| !src.is_empty())
				.map(|src| Page { content: PageContent::url(src), ..Default::default() })
				.collect()
		})
		.unwrap_or_default()
}

// MARK: Text

fn first_number(value: &str) -> Option<f32> {
	parse_chapter_name(value).0
}

/// "Capitolo 12.5 - Titolo" → 12.5 and "Titolo"; "Oneshot" → no number, title "Oneshot".
pub fn parse_chapter_name(name: &str) -> (Option<f32>, Option<String>) {
	let start = match name.find(|c: char| c.is_ascii_digit()) {
		Some(start) => start,
		None => {
			let title = name.trim();
			return (None, if title.is_empty() { None } else { Some(title.into()) });
		}
	};
	let digits: String = name[start..]
		.chars()
		.take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
		.collect();
	let digits = digits.trim_end_matches(['.', ',']);
	let number = digits.replace(',', ".").parse::<f32>().ok();
	let rest = name[start + digits.len()..]
		.trim_start()
		.trim_start_matches(['-', '–', ':', '.'])
		.trim();
	(number, if rest.is_empty() { None } else { Some(rest.to_string()) })
}

const MONTHS: [&str; 12] = [
	"gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno", "luglio", "agosto", "settembre", "ottobre",
	"novembre", "dicembre",
];

/// "22 Novembre 2026" → seconds since 1970 at midnight UTC.
pub fn parse_italian_date(value: &str) -> Option<i64> {
	let mut parts = value.split_whitespace();
	let day: i64 = parts.next()?.parse().ok()?;
	let month_name = parts.next()?.to_lowercase();
	let month = MONTHS.iter().position(|m| *m == month_name)? as i64 + 1;
	let year: i64 = parts.next()?.parse().ok()?;
	Some(days_from_civil(year, month, day) * 86_400)
}

/// "2026-09-05T15:32:50+02:00" → seconds since 1970.
pub fn parse_iso_date(value: &str) -> Option<i64> {
	let (date, rest) = value.split_once('T')?;
	let mut ymd = date.split('-').map(|p| p.parse::<i64>().ok());
	let (year, month, day) = (ymd.next()??, ymd.next()??, ymd.next()??);
	let time = rest.get(0..8)?;
	let mut hms = time.split(':').map(|p| p.parse::<i64>().ok());
	let (h, m, s) = (hms.next()??, hms.next()??, hms.next()??);
	let zone = &rest[8..];
	let offset = if zone.len() >= 6 && (zone.starts_with('+') || zone.starts_with('-')) {
		let sign = if zone.starts_with('-') { -1 } else { 1 };
		let oh: i64 = zone.get(1..3)?.parse().ok()?;
		let om: i64 = zone.get(4..6)?.parse().ok()?;
		sign * (oh * 3600 + om * 60)
	} else {
		0
	};
	Some(days_from_civil(year, month, day) * 86_400 + h * 3600 + m * 60 + s - offset)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
	let y = if month <= 2 { year - 1 } else { year };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = y - era * 400;
	let mp = (month + 9) % 12;
	let doy = (153 * mp + 2) / 5 + day - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	era * 146_097 + doe - 719_468
}
