mod fixtures;

use rusqlite::{Connection, Result as SqliteResult};
use serde::Serialize;
use std::env;

#[derive(Debug, Serialize)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub department: String,
    pub winner: String,
    pub secret_winner: String,
}

#[derive(Debug, Serialize)]
pub struct Tip {
    pub id: i32,
    pub user_id: i32,
    pub match_id: i32,
    pub score_home: i32,
    pub score_away: i32,
}

#[derive(Debug, Serialize)]
pub struct Game {
    pub id: i32,
    pub home_team: String,
    pub away_team: String,
    pub home_score: i32,
    pub away_score: i32,
    pub date: i64,
}

// Opens a single connection. Callers reuse one connection per request rather
// than opening one per query. Env (.env) is loaded ONCE in main(), never here,
// so this stays free of file I/O and the global env lock on the hot path.
pub fn establish_connection() -> SqliteResult<Connection> {
    let mode = env::var("MODE").unwrap_or_else(|_| String::from("production"));
    let conn = if mode == "test" {
        let connection = Connection::open_in_memory()?;
        fixtures::load_fixtures(&connection);
        connection
    } else {
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let connection = Connection::open(database_url)?;
        // Shared DB with frontend + macht-api: wait for the lock instead of
        // failing with SQLITE_BUSY, and use WAL so readers don't block writers.
        connection.busy_timeout(std::time::Duration::from_millis(5000))?;
        connection.query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        })?;
        connection
    };

    Ok(conn)
}

pub fn get_users(conn: &Connection) -> SqliteResult<Vec<User>> {
    let mut stmt =
        conn.prepare("SELECT id, username, department, winner, secretWinner FROM user")?;

    let user_iter = stmt.query_map([], |row| {
        Ok(User {
            id: row.get(0)?,
            username: row.get(1)?,
            department: row.get(2)?,
            winner: row.get(3)?,
            secret_winner: row.get(4)?,
        })
    })?;

    let mut user_list = Vec::new();
    for user in user_iter {
        user_list.push(user?);
    }

    Ok(user_list)
}

// Every still-countable tip (placed before kickoff, BA-003) for all users in a
// single query, so the rating computation avoids an N+1 query per user.
pub fn get_all_tips(conn: &Connection) -> SqliteResult<Vec<Tip>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.user_id, t.match_id, t.score_home, t.score_away \
         FROM tip t JOIN match m ON m.id = t.match_id \
         WHERE t.date < m.utcDate \
         ORDER BY t.id",
    )?;

    let tips_iter = stmt.query_map([], |row| {
        Ok(Tip {
            id: row.get(0)?,
            user_id: row.get(1)?,
            match_id: row.get(2)?,
            score_home: row.get(3)?,
            score_away: row.get(4)?,
        })
    })?;

    let mut tips_list = Vec::new();
    for tip in tips_iter {
        tips_list.push(tip?);
    }

    Ok(tips_list)
}

pub fn get_past_games(conn: &Connection) -> SqliteResult<Vec<Game>> {
    let mut stmt = conn.prepare("SELECT id, homeTeam, awayTeam, homeScore, awayScore, utcDate FROM match WHERE homeScore >= 0 AND awayScore >= 0")?;

    let game_iter = match stmt.query_map([], |row| {
        Ok(Game {
            id: row.get(0)?,
            home_team: row.get(1)?,
            away_team: row.get(2)?,
            home_score: row.get(3)?,
            away_score: row.get(4)?,
            date: row.get(5)?,
        })
    }) {
        Ok(game_iter) => game_iter,
        Err(_) => return Ok(Vec::new()),
    };

    let game_list: Result<Vec<_>, _> = game_iter.collect();
    match game_list {
        Ok(game_list) => Ok(game_list),
        Err(_) => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::Team;
    use serde_json::from_str;

    #[test]
    fn test_get_users() {
        env::set_var("MODE", "test");
        let conn = establish_connection().unwrap();
        let users = get_users(&conn).unwrap();
        assert_eq!(users.len(), 7);

        assert_eq!(users[0].username, "JohnDoe");
        assert_eq!(users[0].department, "Langenfeld");
        assert_eq!(users[0].id, 1);

        assert_eq!(users[1].username, "ToniKroos");
        assert_eq!(users[1].department, "Langenfeld");
        assert_eq!(users[1].id, 2);

        assert_eq!(users[6].username, "SteveMcManaman");
        assert_eq!(users[6].department, "London");
        assert_eq!(users[6].id, 7);
    }

    #[test]
    fn test_get_all_tips_for_user() {
        env::set_var("MODE", "test");
        let conn = establish_connection().unwrap();
        let tips: Vec<Tip> = get_all_tips(&conn)
            .unwrap()
            .into_iter()
            .filter(|t| t.user_id == 1)
            .collect();
        assert_eq!(tips.len(), 2);

        assert_eq!(tips[0].id, 1);
        assert_eq!(tips[0].user_id, 1);
        assert_eq!(tips[0].match_id, 1);
        assert_eq!(tips[0].score_home, 2);
        assert_eq!(tips[0].score_away, 0);

        assert_eq!(tips[1].id, 2);
        assert_eq!(tips[1].user_id, 1);
        assert_eq!(tips[1].match_id, 2);
        assert_eq!(tips[1].score_home, 1);
        assert_eq!(tips[1].score_away, 0);
    }

    #[test]
    fn test_get_all_tips_excludes_post_kickoff_tips() {
        env::set_var("MODE", "test");
        let conn = establish_connection().unwrap();
        let tips: Vec<Tip> = get_all_tips(&conn)
            .unwrap()
            .into_iter()
            .filter(|t| t.user_id == 6)
            .collect();
        assert!(
            tips.iter().any(|t| t.match_id == 1),
            "pre-kickoff tip on match 1 must remain scored"
        );
        assert!(
            !tips.iter().any(|t| t.match_id == 2),
            "post-kickoff tip on match 2 must be excluded from scoring"
        );
    }

    #[test]
    fn test_get_past_games() {
        env::set_var("MODE", "test");
        let conn = establish_connection().unwrap();
        let games = get_past_games(&conn).unwrap();
        assert_eq!(games.len(), 2);

        assert_eq!(games[0].id, 1);
        assert_eq!(games[0].home_score, 2);
        assert_eq!(games[0].away_score, 0);

        let home_team: Team = from_str(&games[0].home_team).unwrap();
        assert_eq!(home_team.name, "Germany");
        assert_eq!(home_team.tla, "GER");

        let away_team: Team = from_str(&games[0].away_team).unwrap();
        assert_eq!(away_team.name, "Spain");
        assert_eq!(away_team.tla, "ESP");

        assert_eq!(games[1].id, 2);
        assert_eq!(games[1].home_score, 1);
        assert_eq!(games[1].away_score, 1);

        let home_team: Team = from_str(&games[1].home_team).unwrap();
        assert_eq!(home_team.name, "Poland");
        assert_eq!(home_team.tla, "POL");

        let away_team: Team = from_str(&games[1].away_team).unwrap();
        assert_eq!(away_team.name, "France");
        assert_eq!(away_team.tla, "FRA");
    }
}
