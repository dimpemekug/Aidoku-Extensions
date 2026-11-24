// Offline tests on pages saved from www.mangaworld.me (scripts, styles and icons removed).
// Run with `cargo test` (needs aidoku-test-runner).

use crate::parse::*;
use crate::{archive_url, sorted_archive};
use aidoku::{
	ContentRating, FilterValue, MangaStatus, PageContent, Viewer,
	alloc::{String, vec},
	imports::html::{Document, Html},
};
use aidoku_test::aidoku_test;

const BASE: &str = "https://www.mangaworld.me";

fn page(html: &str) -> Document {
	Html::parse_with_url(html, BASE).expect("la pagina di prova si legge")
}

fn home() -> Document {
	page(include_str!("../tests/fixtures/home.html"))
}

fn series() -> Document {
	page(include_str!("../tests/fixtures/manga.html"))
}

// MARK: Indirizzi

#[aidoku_test]
fn l_id_di_una_serie_e_il_numero_dopo_manga() {
	assert_eq!(manga_id_from_url("https://www.mangaworld.me/manga/1708/one-piece").as_deref(), Some("1708"));
	assert_eq!(manga_id_from_url("https://www.mangaworld.me/archive"), None);
}

#[aidoku_test]
fn l_id_di_un_capitolo_e_quello_dopo_read() {
	let url = "https://www.mangaworld.me/manga/1708/one-piece/read/6b032279179d77b59c06a674";
	assert_eq!(chapter_id_from_url(url).as_deref(), Some("6b032279179d77b59c06a674"));
}

#[aidoku_test]
fn il_dominio_scritto_a_mano_diventa_un_indirizzo_completo() {
	assert_eq!(normalized_domain(" www.mangaworld.xx/ "), "https://www.mangaworld.xx");
	assert_eq!(normalized_domain("https://www.mangaworld.me"), "https://www.mangaworld.me");
	assert_eq!(normalized_domain("   "), "");
}

#[aidoku_test]
fn la_ricerca_porta_parola_generi_esclusi_e_pagina() {
	let filters = vec![
		FilterValue::Sort { id: "sort".into(), index: 0, ascending: false },
		FilterValue::MultiSelect {
			id: "genre".into(),
			included: vec![String::from("shounen")],
			excluded: vec![String::from("ecchi")],
		},
	];
	let url = archive_url(BASE, 2, Some("one piece"), &filters);
	assert!(url.starts_with("https://www.mangaworld.me/archive?"));
	assert!(url.contains("keyword=one%20piece") || url.contains("keyword=one+piece"));
	assert!(url.contains("genre=shounen"));
	assert!(url.contains("exclude=ecchi"));
	assert!(url.contains("page=2"));
	assert!(!url.contains("sort="), "con una parola cercata vale la pertinenza del sito");
}

#[aidoku_test]
fn le_liste_ordinate_usano_l_ordinamento_del_sito() {
	assert_eq!(sorted_archive(BASE, "top_rated", 1), "https://www.mangaworld.me/archive?sort=top_rated");
}

// MARK: Archivio

#[aidoku_test]
fn una_pagina_piena_da_20_serie_e_la_pagina_successiva() {
	let result = parse_archive(&page(include_str!("../tests/fixtures/archive-page2.html")));
	assert_eq!(result.entries.len(), 20);
	assert!(result.has_next_page);
	let first = &result.entries[0];
	assert!(first.key.chars().all(|c| c.is_ascii_digit()));
	assert!(first.cover.as_deref().unwrap_or("").starts_with("https://cdn.mangaworld.me/"));
	assert!(!first.cover.as_deref().unwrap_or("").contains('?'), "il parametro anti-cache va tolto");
}

#[aidoku_test]
fn una_ricerca_con_pochi_risultati_non_ha_pagina_successiva() {
	let result = parse_archive(&page(include_str!("../tests/fixtures/archive-search.html")));
	assert_eq!(result.entries.len(), 7);
	assert!(!result.has_next_page);
	assert!(result.entries.iter().any(|m| m.title == "Naruto"));
}

// MARK: Home

#[aidoku_test]
fn le_tendenze_mostrano_ogni_serie_una_volta_sola() {
	let trending = parse_trending(&home());
	assert!(!trending.is_empty());
	assert_eq!(trending[0].title, "One Piece");
	let mut keys: aidoku::alloc::Vec<&String> = trending.iter().map(|m| &m.key).collect();
	keys.dedup();
	assert_eq!(keys.len(), trending.len());
}

#[aidoku_test]
fn gli_ultimi_capitoli_portano_serie_capitolo_e_pagina_successiva() {
	let (entries, has_next) = parse_latest(&home());
	assert_eq!(entries.len(), 16);
	assert!(has_next);
	assert_eq!(entries[0].manga.key, "3149");
	assert_eq!(entries[0].chapter.key, "6b037bca8a6985bfee08bbf6");
	assert_eq!(entries[0].chapter.chapter_number, Some(72.0));
}

#[aidoku_test]
fn la_seconda_pagina_degli_ultimi_capitoli_e_diversa_dalla_prima() {
	let (first, _) = parse_latest(&home());
	let (second, _) = parse_latest(&page(include_str!("../tests/fixtures/home-page2.html")));
	assert!(!second.is_empty());
	assert_ne!(first[0].manga.key, second[0].manga.key);
}

// MARK: Serie

#[aidoku_test]
fn i_dettagli_hanno_titolo_copertina_trama_autori_stato_e_generi() {
	let manga = parse_details(&series(), BASE, aidoku::Manga { key: "1708".into(), ..Default::default() });
	assert_eq!(manga.title, "One Piece");
	assert!(manga.cover.as_deref().unwrap_or("").ends_with("/mangas/5fa0c9e2c9f2201ee55d3bd4.jpg"));
	assert!(manga.description.as_deref().unwrap_or("").starts_with("Monkey D. Rufy"));
	assert_eq!(manga.authors, Some(vec![String::from("ODA Eiichiro")]));
	assert_eq!(manga.status, MangaStatus::Ongoing);
	assert_eq!(manga.content_rating, ContentRating::Safe);
	assert_eq!(manga.viewer, Viewer::RightToLeft);
	assert!(manga.tags.unwrap_or_default().iter().any(|t| t == "Shounen"));
	assert_eq!(manga.url.as_deref(), Some("https://www.mangaworld.me/manga/1708"));
}

#[aidoku_test]
fn i_capitoli_escono_dal_piu_nuovo_con_volume_e_data() {
	let chapters = parse_chapters(&series(), BASE, "1708");
	assert_eq!(chapters.len(), 1201);
	let newest = &chapters[0];
	assert_eq!(newest.key, "6b032279179d77b59c06a674");
	assert_eq!(newest.chapter_number, Some(1199.0));
	assert_eq!(newest.volume_number, Some(116.0));
	assert_eq!(newest.date_uploaded, Some(1_795_305_600)); // 22 novembre 2026
	assert_eq!(chapters[chapters.len() - 1].chapter_number, Some(0.0));
}

#[aidoku_test]
fn le_pagine_di_un_capitolo_sono_le_immagini_del_lettore_in_ordine() {
	let pages = parse_pages(&page(include_str!("../tests/fixtures/chapter.html")));
	assert_eq!(pages.len(), 13);
	match &pages[0].content {
		PageContent::Url(url, _) => assert!(url.ends_with("/capitolo-1199-6b032279179d77b59c06a674/1.jpg")),
		_ => panic!("la prima pagina deve essere un indirizzo"),
	}
}

// MARK: Testi

#[aidoku_test]
fn un_nome_di_capitolo_si_divide_in_numero_e_titolo() {
	assert_eq!(parse_chapter_name("Capitolo 12.5 - Il ritorno"), (Some(12.5), Some(String::from("Il ritorno"))));
	assert_eq!(parse_chapter_name("Capitolo 7"), (Some(7.0), None));
	assert_eq!(parse_chapter_name("Oneshot"), (None, Some(String::from("Oneshot"))));
}

#[aidoku_test]
fn le_date_italiane_e_iso_diventano_secondi() {
	assert_eq!(parse_italian_date("05 Gennaio 2025"), Some(1_736_035_200));
	assert_eq!(parse_italian_date("ieri"), None);
	assert_eq!(parse_iso_date("2026-09-05T15:32:50+02:00"), Some(1_788_615_170));
}

#[aidoku_test]
fn i_generi_per_adulti_o_maturi_alzano_la_classificazione() {
	assert_eq!(rating_for_genres(&["azione"]), ContentRating::Safe);
	assert_eq!(rating_for_genres(&["azione", "ecchi"]), ContentRating::Suggestive);
	assert_eq!(rating_for_genres(&["ecchi", "hentai"]), ContentRating::NSFW);
}
