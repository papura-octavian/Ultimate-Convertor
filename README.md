<p align="center">
  <img src="src/assets/icon.svg" alt="" width="96">
</p>

<h1 align="center">Ultimate Convertor</h1>

<p align="center">
  Convertește orice, rapid: imagini, video, audio, date și documente, plus unități, numere și culori.<br>
  Parte din <a href="https://skipit.space">SkipIt</a>.
</p>

---

Tragi fișiere sau foldere întregi în fereastră, alegi formatul și apeși **Convertește**. Motorul e scris în **Rust** și lucrează în paralel pe toate nucleele, iar video-ul care are deja codec-ul potrivit e doar re-ambalat, în câteva secunde în loc de minute.

## Ce convertește

| Categorie | Din | În |
|---|---|---|
| Imagini | HEIC, HEIF, AVIF, WebP, PNG, JPG, GIF, TIFF, BMP, ICO, TGA, QOI, PNM, HDR, EXR, DDS | JPG, PNG, WebP, AVIF, GIF, BMP, TIFF, ICO, TGA, QOI, **PDF** |
| Video | MP4, MKV, MOV, AVI, WebM, WMV, FLV, MPG, TS/MTS/M2TS, 3GP, OGV, VOB… | MP4, MKV, WebM, MOV, AVI, **GIF**, și audio extras: MP3, M4A, WAV, FLAC, OGG, Opus |
| Audio | MP3, WAV, FLAC, M4A, AAC, OGG, Opus, WMA, AIFF, AMR, AC3, APE, WV… | MP3, M4A, WAV, FLAC, OGG, Opus, AIFF |
| Date | JSON, YAML, TOML, CSV, TSV | JSON, YAML, TOML, CSV |
| Documente | Markdown | HTML |

GIF-urile animate pot deveni și MP4/WebM.

## Funcții

- **Drag & drop** pentru fișiere și foldere (cu tot cu subfoldere); tot ce nu are un format cunoscut e ignorat
- **Format pe grup**: un clic pe „JPG” la Imagini schimbă toate imaginile odată; fiecare fișier se poate schimba și separat
- **Calitate** (un singur slider pentru JPG, WebP, AVIF, video și bitrate audio) și **dimensiune maximă** (4K, 1920, 1280 sau orice valoare)
- **Mod rapid**: MKV → MP4, MP4 → M4A și altele similare copiază stream-urile fără re-encodare
- **Un singur PDF** din mai multe imagini, în ordinea din listă
- Salvare lângă fișierul original sau într-un folder ales; numele existente nu sunt suprascrise niciodată (`poza (2).jpg`)
- Bară de progres reală pentru video/audio, buton de oprire, „Arată în folder”, cât a scăzut fișierul
- Orientarea EXIF a pozelor de pe telefon e aplicată automat
- **Unități**: lungime, masă, temperatură, volum, suprafață, viteză, timp, date digitale, presiune, energie, putere, unghi, consum
- **Numere**: zecimal, hexazecimal, binar, octal, roman (fără limită de mărime), și Text ↔ Base64
- **Culori**: HEX, RGB, HSL, HSV, CMYK, ARGB, contrast WCAG și nuanțe
- Interfață în **română** și **engleză**
- Scurtături: `Ctrl+O` adaugă fișiere · `Ctrl+Enter` convertește / oprește

## Instalare

Descarcă `UltimateConvertor-Setup-x.y.z.exe` din [Releases](../../releases/latest) și rulează-l. Nu cere drepturi de administrator. Merge pe Windows 10 și 11 (inclusiv edițiile N, fără Media Feature Pack: ffmpeg e inclus).

## Arhitectură pe scurt

- **[Tauri 2](https://tauri.app)**: fereastra nativă cu WebView2, installer mic (~30 MB cu ffmpeg cu tot)
- **Rust** ([src-tauri/src](src-tauri/src))
  - [`lib.rs`](src-tauri/src/lib.rs): comenzile, planificarea numelor de ieșire, două cozi de lucru (video-ul greu pe 2 fire, restul pe câte un fir pe nucleu), evenimente de progres, oprire
  - [`images.rs`](src-tauri/src/images.rs): decodare/encodare cu crate-ul `image` și `libwebp`; HEIC/AVIF se decodează prin ffmpeg
  - [`media.rs`](src-tauri/src/media.rs): ffmpeg (inclus ca sidecar): probe, remux, encodare, progres din `-progress pipe:1`
  - [`pdf.rs`](src-tauri/src/pdf.rs): un scriitor PDF minimal (o pagină JPEG per imagine)
  - [`data.rs`](src-tauri/src/data.rs): JSON/YAML/TOML/CSV printr-un model comun, Markdown → HTML
- **UI** ([src](src)): HTML/CSS/JS simplu, fără framework și fără build, deci pornește instant

## Dezvoltare

Cerințe: [Rust](https://rustup.rs), Node.js 20+, Visual Studio Build Tools (C++).

ffmpeg nu e în repo. Pune un `ffmpeg.exe` în `src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe` (de exemplu din [gyan.dev](https://www.gyan.dev/ffmpeg/builds/), varianta „release essentials”).

```bash
npm install
npm run dev        # pornește aplicația
npm test           # testele de conversie (imagini, video, audio, date, PDF, oprire)
npm run build      # installer-ul NSIS în src-tauri/target/release/bundle/nsis
```

## Release

Workflow-ul [`.github/workflows/release.yml`](.github/workflows/release.yml) descarcă ffmpeg, rulează testele, construiește installer-ul și îl publică la fiecare tag de versiune:

```bash
git tag v1.0.1
git push origin v1.0.1
```

Versiunea se schimbă în `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` și `package.json`.

## Licență

[MIT](LICENSE). Installer-ul include [FFmpeg](https://ffmpeg.org) (GPL), ca program separat; sursa și licența lui sunt pe [ffmpeg.org](https://ffmpeg.org/legal.html).
