# Sviluppo

Note per chi modifica le fonti di questo repository.

## Struttura

```
sources/it.mangaworldita/
├── Cargo.toml
├── .cargo/config.toml   target wasm32 e runner dei test
├── res/
│   ├── source.json      id, nome, versione, sito, liste
│   ├── filters.json     ordinamenti, generi, tipologie, stati, autore, artista
│   ├── settings.json    campo Dominio
│   └── icon.png         128×128, da assets/icon-mangaworld.svg
├── src/
│   ├── lib.rs           la fonte: ricerca, Home, liste, immagini, link diretti
│   ├── parse.rs         lettura dell'HTML, senza richieste (provabile offline)
│   └── tests.rs         test offline sulle pagine salvate
└── tests/fixtures/      pagine di www.mangaworld.me senza script, stili e icone
site/index.html          pagina del repository (pulsanti, indirizzo, elenco fonti)
site/icon.png            icona del repository, da assets/icon-extensions.svg
assets/                  icone in SVG
```

## Strumenti

```bash
brew install rustup
rustup-init -y --default-toolchain stable --target wasm32-unknown-unknown
cargo install --locked --git https://github.com/Aidoku/aidoku-rs aidoku-cli
cargo install --locked --git https://github.com/Aidoku/aidoku-rs aidoku-test-runner
```

## Comandi (dalla cartella della fonte)

```bash
cargo test                 # test offline con aidoku-test-runner
aidoku package             # crea package.aix
aidoku verify package.aix  # controlla icona, manifest, filtri, impostazioni
```

Dalla radice: `aidoku build sources/*/package.aix --name "Aidoku Extensions"` crea `public/`
(`index.min.json`, icone, pacchetti); il workflow ci copia `site/index.html` e pubblica su GitHub Pages.

Icona della fonte: `rsvg-convert -w 128 -h 128 assets/icon-mangaworld.svg -o sources/it.mangaworldita/res/icon.png`

Icona del repository (pagina, favicon e README), generica per tutte le fonti:
`rsvg-convert -w 256 -h 256 assets/icon-extensions.svg -o site/icon.png && cp site/icon.png assets/icon-extensions.png`

## Pubblicare una nuova versione

Alza `version` in `res/source.json` e fai push su `master`: le app vedono l'aggiornamento solo se la
versione cambia. Se il sito cambia tema, aggiorna i selettori in `src/parse.rs` e le pagine in
`tests/fixtures/`.

## Cosa resta fuori

- **Prove sul sito vero in CI**: il sito può bloccare gli indirizzi di GitHub Actions, quindi la CI
  esegue solo i test offline. Prima di un rilascio la fonte va provata in un'app.
- **Bookmark e voti del sito**: servono l'account MangaWorld.
- **MangaWorldAdult**: è un sito a parte, non incluso.

## Controlli a mano nell'app

1. Aggiungi il repository e installa *MangaWorld ITA*.
2. Home: tendenze, ultimi capitoli, più letti, ultime aggiunte e più votati; "Vedi tutto" apre la lista e scorre.
3. Cerca "one piece", prova un filtro di genere ed escludine uno.
4. Apri la serie, controlla trama, generi e numero di capitoli, leggi un capitolo fino in fondo.
5. Incolla un link di MangaWorld in "Apri link".
6. Impostazioni della fonte: cambia il dominio e poi svuotalo.
