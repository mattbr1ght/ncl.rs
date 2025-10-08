# Plan pracy 

| Zadanie                                  | Opis                                                                                        | Czas (h) |
| ---------------------------------------- | ------------------------------------------------------------------------------------------- | -------- |
| Inicjalizacja projektu CLI               | Utworzenie projektu Cargo i podstawowej struktury katalogów                                 | 1        |
| Implementacja parsowania argumentów      | Konfiguracja clap i routingu komend `init`, `check`, `template`, `run`, `task`              | 3        |
| Interaktywny kreator                     | Implementacja menu wyboru stacku i opcji przy użyciu inquire lub cliclack                   | 3        |
| Generator struktury projektu             | Implementacja tworzenia katalogów projektu, `.data` i plików README, NOTES                  | 3        |
| Generator minimalnego kodu projektu      | Generowanie plików startowych dla PHP, Next.js i React Native                               | 3        |
| Szablony docker-compose dla stacków      | Przygotowanie szablonów docker-compose dopasowanych do każdego wspieranego stacku           | 3        |
| System szablonów                         | Wczytywanie i instalacja szablonów z `~/.ncl/templates`                                     | 3        |
| Parsowanie metadanych szablonu           | Wczytywanie i walidacja plików TOML/YAML z metadanymi szablonu                              | 4        |
| Weryfikacja środowiska                   | Implementacja sprawdzania obecności `docker`, `git`, `php`, `node` i komunikatów o brakach  | 3        |
| Integracja z Gitem                       | Implementacja `git init`, pierwszy commit oraz opcjonalne `git remote add`                  | 2        |
| Menedżer zadań                           | Implementacja skanera TODO/FIXME/NOTE i podstawowego listowania zadań                       | 3        |
| Filtrowanie i otwieranie zadań           | Implementacja filtrów wyników oraz otwierania plików z kontekstem                           | 2        |
| Integracja z GitHub/GitLab (opcjonalnie) | Implementacja synchronizacji z Issue przez API jako opcja konfigurowalna                    | 3        |
| Task runner `ncl run`                    | Implementacja uruchamiania zdefiniowanych skryptów typu dev/build/deploy                    | 2        |
| Generowanie plików CI/CD                 | Generowanie prostego pliku CI/CD (`.gitlab-ci.yml` lub `.github/workflows/ci.yml`)          | 3        |
| Testy jednostkowe                        | Napisanie testów dla kluczowych modułów: generatora, parsera, skanera                       | 3        |
| Refaktoryzacja i poprawki                | Poprawa UX CLI, obsługa błędów i  poprawki                                                  | 5        |
| Przygotowanie przykładowych szablonów    | Stworzenie podstawowych szablonów dla WordPress, PrestaShop, Laravel, Next.js, React Native | 3        |
| Pakowanie i dystrybucja binarki          | Przygotowanie skryptu budowania i podstaw cross-compilation                                 | 1        |

Suma godzin: 53

---

## Lista zadań 

- [ ] Inicjalizacja projektu CLI
- [ ] Implementacja parsowania argumentów
- [ ] Interaktywny kreator
- [ ] Generator struktury projektu
- [ ] Generator minimalnego code scaffold
- [ ] Szablony docker-compose dla stacków
- [ ] System szablonów
- [ ] Parsowanie metadanych szablonu
- [ ] Weryfikacja środowiska
- [ ] Integracja z Gitem
- [ ] Menedżer zadań (TODO scanner)
- [ ] Filtrowanie i otwieranie zadań
- [ ] Integracja z GitHub/GitLab (opcjonalnie)
- [ ] Task runner `ncl run`
- [ ] Generowanie stubu CI
- [ ] Testy jednostkowe
- [ ] Refaktoryzacja i poprawki
- [ ] Przygotowanie przykładowych szablonów
- [ ] Pakowanie i dystrybucja binarki
