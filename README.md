# **NCL - Nice Code Launcher**

## 1. Cel projektu

NCL to wewnętrzne narzędzie CLI służące do szybkiego i powtarzalnego tworzenia szkiele­tów projektów w firmie. Umożliwia:

- wygenerowanie struktury projektu zgodnej ze standardami firmy,
- przygotowanie środowiska (docker-compose),
- inicjalizację repozytorium Git,
- utworzenie katalogu `.data` z dokumentacją i notatkami,
- wczytanie własnych szablonów projektów,
- prosty menedżer zadań (komentarze TODO  i ewentualnie GitHub/Gitlab issues),
- uruchamianie skryptów / zadań takich jak np. `ncl run dev`

### 1.1 Nazwa narzędzia

Nazwa NCL - Nice Code Launcher - nawiązuje do "wystartowania" nowego projektu jak statku kosmicznego, ale też do angielskiego określenia na wprowadzenie nowego produktu przez firmę

## 2. Technologie

- Język: Rust (szybki, bezpieczny, idealny do narzędzi CLI)
  - Biblioteki:
    - clap - parsowanie argumentów CLI
    - inquire / cliclack - interaktywne menu terminalowe
    - serde / toml / yaml / json - konfiguracje i szablony
    - tokio / std::process - asynchroniczność i wykonywanie komend systemowych (git, docker)
  - Opcjonalne biblioteki:
    - validator - walidacja danych wejściowych
    - reqwest / octocrab - integracja z API GitHub/GitLab

## 3. Design

- Program uruchamiany lokalnie jako pojedynczy binarny plik CLI
- Dwa tryby pracy: nieinteraktywny (argumenty) i interaktywny (menu wyboru)
- Komunikaty tekstowe: czytelne, z jasnymi instrukcjami dalszych kroków
- Modułowa architektura ułatwiająca dodawanie nowych szablonów i funkcji

## 4. Główne moduły i funkcje

### 4.1 Core

- Parsowanie argumentów i routing komend: `init`, `check`, `template`, `run`, `task`
- Tryb skryptowy i tryb interaktywny

### 4.2 Generator projektu

- Tworzenie katalogu projektu i katalogu `.data` (README.md, NOTES.md)
- Generowanie minimalnej struktury i kodu scaffold (plików startowych) dla obsługiwanych stacków:
  - PHP (WordPress / PrestaShop / Laravel): `HomeController.php`, `views/home.blade.php` lub `index.php` dla WordPress.
  - Next.js: `pages/index.tsx`, `components/Hello.tsx`
  - React Native: `App.js`, `components/Hello.js`
- Generowanie `docker-compose.yml` dopasowanego do wybranego szablonu (serwer aplikacji, baza danych, serwer statyczny)
- Dodawanie opcjonalnych narzędzi np. ESLint, Prettier
- Tworzenie `README.md` z checklistą uruchomienia i plików `.env`

### 4.3 System szablonów

- Wczytywanie szablonów z `$HOME/.ncl/templates/`
- Metadane szablonu w formacie TOML/YAML: nazwa, opis, wymagane narzędzia, zdefiniowane zadania do uruchomienia przy użyciu `ncl run`, pliki/foldery do skopiowania
- Mechanizm instalacji i aktualizacji szablonów

### 4.4 Weryfikacja środowiska

- Weryfikacja obecności: `docker`, `git`, `php`, `node`, `docker-compose` itp.
- Wyświetlenie braków z krótkimi instrukcjami instalacji
- Opcjonalna instalacja brakujących komponentów

### 4.5 Git integration

- `git init` oraz pierwszy commit inicjalizacyjny.
- Opcjonalne ustawienie zdalnego origin (`git remote add origin <url>`), gdy podane lub w integracji z API.
- Możliwość wygenerowania stubu pliku CI (`.gitlab-ci.yml` lub `.github/workflows/ci.yml`).

### 4.6 Skrypty / zadania

- Wykonywanie skryptów po generacji projektu
- Definiowanie zadań i często wykonywanych "jobów" (`ncl run dev`, `ncl run build`, `ncl run deploy`)

### 4.7 Menedżer zadań

- Skanowanie kodu pod kątem komentarzy TODO/NOTE/FIXME
- Wyświetlanie listy zadań z lokalizacją pliku i numerem linii
- Filtrowanie wyników (folder, typ komentarza, priorytet jeśli występuje)
- Opcjonalna integracja z GitHub/GitLab Issues

### 4.8 Integracja ze zdalnym hostem repozytorium (opcjonalnie)

- Tworzenie repozytorium przez API GitHub/GitLab
- Automatyczne ustawienie `git remote add origin`

## 5. Przykładowy workflow użytkownika

### Przygotowanie środowiska (jednorazowo)

1. Instalacja NCL (jednorazowo, lokalnie)
2. Umieszczenie własnych szablonów w `~/.ncl/templates/` (opcjonalnie)

### Tworzenie nowego projektu (przykład PrestaShop)

1. Użytkownik uruchamia `ncl init  sklep-presta` lub `ncl init`
2. Wybór nazwy, wybór szablonu i ewentualnych opcji konfiguracyjnych
3. Uruchomienie environment check i informuje o brakach
4. Stworzenie katalogu projektu, generacja `.data/`, plików startowych i konfiguracja`docker-compose.yml`
5. Wykonanie `git init` i pierwszy commit
6. Wyświetlenie następnych kroków (instrukcje) np. `cd PROJECT/ & ncl run dev`

### Praca z menadżerem zadań

1. `ncl task list` — listuje lokalne TODO/FIXME i ewentualnie zsynchronizowane GitHub/Gitlab Issues
2. `ncl task open <id>` — otwiera plik w edytorze (jeżeli skonfigurowany) lub wyświetla kontekst
3. `ncl task sync --github` — synchronizuje lokalne zadania z Issues

## 6. Przykładowe flow (skrótowo)

1. `ncl init` → wybór szablonu → environment check
2. Generacja struktury → docker-compose → git init
3.  Dalsze instrukcje
4. `ncl task list` → przegląd TODO → ewentualna synchronizacja ze zdalnym hostem

## 7. Struktura wygenerowanego projektu (przykład)

```
projekt-nazwa/
├─ .data/
│  ├─ README.md
│  ├─ NOTES.md
├─ src/ (lub app/ zależnie od szablonu)
├─ docker-compose.yml
├─ .env
├─ .git/
├─ .gitignore
├─ README.md
```

## 8. Przykładowa struktura projektu CLI

```
ncl/
├─ src/
│  ├─ main.rs
│  ├─ commands/
│  │   ├─ init.rs
│  │   ├─ check.rs
│  │   ├─ run.rs
│  ├─ modules/
│  │   ├─ generator.rs
│  │   ├─ docker.rs
│  │   ├─ git.rs
│  │   ├─ templates.rs
│  │   ├─ hooks.rs
│  │   ├─ task_manager.rs
├─ templates/
│  ├─ prestashop/
│  ├─ wordpress/
│  ├─ laravel/
│  ├─ nextjs/
│  ├─ react_native/
└─ README.md
```
