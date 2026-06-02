# betting-api

[![betting-api-ci](https://github.com/football-betting/betting-api/actions/workflows/main.yml/badge.svg)](https://github.com/football-betting/betting-api/actions/workflows/main.yml)
[![codecov](https://codecov.io/gh/football-betting/betting-api/branch/main/graph/badge.svg)](https://codecov.io/gh/football-betting/betting-api)

Read-only HTTP API for the office football-prediction game, built with Rust and
Actix Web. It serves the leaderboard, per-user ratings, and per-game tips that
the Next.js frontend renders. Replaces the archived
[em2024-api](https://github.com/football-betting/em2024-api).

## Architecture

Part of the `football-betting` workspace, alongside the `frontend` (Next.js)
and `macht-api` (importer). All three services share a single SQLite database
at `../shared/db/database.db`.

- **Schema authority** lives in the frontend (`frontend/db/schema.ts`, Drizzle).
  This service only reads — keep its structs in lockstep with that schema.
- This API performs **no writes**. User and tip data is written by the
  frontend; match data is written exclusively by `macht-api`.

## Configuration

The server reads two environment variables:

| Variable       | Purpose                                                        |
|----------------|----------------------------------------------------------------|
| `DATABASE_URL` | Path to the shared SQLite file (e.g. `../shared/db/database.db`). |
| `MODE`         | `production` (default) reads `DATABASE_URL`; `test` uses an in-memory database with fixtures. |

Create a `.env` with at least `DATABASE_URL` set, or export it in the
environment before starting the server.

## Running

```bash
cargo run
```

The server listens on [http://localhost:8080/](http://localhost:8080/).

If you don't have a database yet, point `DATABASE_URL` at the shared
`database.db` produced by the frontend's migrations, or temporarily enable
`fixtures::load_fixtures(&connection)` in `src/db/mod.rs` to seed one on first
start.

## Testing

```bash
cargo test
```

For code coverage:

```bash
cargo tarpaulin --out Html
```

Tests run against an in-memory SQLite database (`MODE=test`) with fixtures, so
they need neither the shared database nor a running server.

## Objects

### UserInfo

Represents information about a user.

- **name**: `string` - The name of the user.
- **user_id**: `i32` - The unique identifier of the user.
- **department**: `string` - The department the user belongs to.
- **position**: `i32` - The position of the user in the ranking.
- **score_sum**: `i32` - The total score of the user.
- **sum_win_exact**: `i32` - The number of exact wins predicted by the user.
- **sum_score_diff**: `i32` - The number of score differences predicted by the user.
- **sum_team**: `i32` - The total points for team predictions.
- **extra_point**: `i32` - Extra points earned by the user.
- **tips**: `Tip[]` - The tips provided by the user.

Example:

```json
{
  "name": "ninja",
  "user_id": 1,
  "department": "Langenfeld",
  "position": 16,
  "score_sum": 6,
  "sum_win_exact": 0,
  "sum_score_diff": 0,
  "sum_team": 6,
  "extra_point": 0,
  "tips": []
}
```

### Tip

Represents a user's prediction for a match.

- **match_id**: `string` - The unique identifier for the match.
- **user**: `string` - The name of the user.
- **user_id**: `i32` - The unique identifier of the user.
- **score**: `i32` - The score given by the user for the match.
- **team1**: `Team` - The first team in the match.
- **team2**: `Team` - The second team in the match.
- **tip_home**: `i32` - The predicted score for the home team.
- **tip_away**: `i32` - The predicted score for the away team.
- **score_home**: `i32` - The actual score for the home team.
- **score_away**: `i32` - The actual score for the away team.
- **date**: `i64` - The timestamp of the match.

Example:

```json
{
  "match_id": "428759",
  "user": "ninja",
  "user_id": 1,
  "score": 1,
  "team1": {
    "name": "Serbia",
    "tla": "SRB"
  },
  "team2": {
    "name": "England",
    "tla": "ENG"
  },
  "tip_home": 0,
  "tip_away": 2,
  "score_home": 0,
  "score_away": 1,
  "date": 1718564400
}
```

### Team

Represents a football team.

- **name**: `string` - The name of the team.
- **tla**: `string` - The three-letter acronym for the team.

Example:

```json
{
  "name": "England",
  "tla": "ENG"
}
```

## API Endpoints

- **[GET] /rating**: Retrieves all users sorted by position. Returns a list of `UserInfo` objects without tips (tips are an empty array).
- **[GET] /user/{user_id}**: Retrieves a user by their user_id. Returns a `UserInfo` object with tips (tips are an array of `Tip`).
- **[GET] /game/{game_id}**: Retrieves all user tips for a specific game. Returns an array of `Tip` objects.
- **[GET] /**: Returns a JSON object with the status: `{ "status": "works" }`.
