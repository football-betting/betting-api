use crate::db::{get_all_tips, Game, Tip, User};
use rusqlite::Connection;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Team {
    #[serde(default, deserialize_with = "string_or_default")]
    pub name: String,
    #[serde(default, deserialize_with = "string_or_default")]
    pub tla: String,
}

fn string_or_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRating {
    pub name: String,
    pub user_id: i32,
    pub department: String,
    pub position: i32,
    pub score_sum: i32,
    pub sum_win_exact: i32,
    pub sum_score_diff: i32,
    pub sum_team: i32,
    pub extra_point: i32,
    pub tips: Vec<MatchInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchInfo {
    pub match_id: String,
    pub user: String,
    pub user_id: i32,
    pub score: i32,
    pub team1: Team,
    pub team2: Team,
    pub tip_home: Option<i32>,
    pub tip_away: Option<i32>,
    pub score_home: Option<i32>,
    pub score_away: Option<i32>,
    pub date: i64,
}

struct ScoreConfig;

impl ScoreConfig {
    pub const NO_WIN_TEAM: i32 = 0;
    pub const WIN_EXACT: i32 = 5;
    pub const WIN_SCORE_DIFF: i32 = 3;
    pub const WIN_TEAM: i32 = 2;
    pub const WINNER_BONUS: i32 = 12;
    pub const SECRET_WINNER_BONUS: i32 = 6;
}

pub fn get_user_rating(
    games: Vec<Game>,
    users: Vec<User>,
    conn: &Connection,
    tournament_winner: &str,
) -> Result<Vec<UserRating>, Box<dyn std::error::Error>> {
    let mut user_rating_list = Vec::new();
    let tournament_winner = tournament_winner.trim();

    // One query for every countable tip, grouped by user — avoids an N+1 query
    // (one tips query per user) on the hot /rating path.
    let mut tips_by_user: HashMap<i32, HashMap<i32, Tip>> = HashMap::new();
    for tip in get_all_tips(conn)? {
        tips_by_user
            .entry(tip.user_id)
            .or_default()
            .insert(tip.match_id, tip);
    }
    let no_tips: HashMap<i32, Tip> = HashMap::new();

    for user in &users {
        // The tournament champion is configured via env (TOURNAMENT_WINNER).
        // Until it is set there is no winner yet, so nobody earns the bonus.
        let mut extra_point = ScoreConfig::NO_WIN_TEAM;
        if !tournament_winner.is_empty() {
            if user.winner == tournament_winner {
                extra_point = ScoreConfig::WINNER_BONUS;
            }
            if user.secret_winner == tournament_winner {
                extra_point = ScoreConfig::SECRET_WINNER_BONUS;
            }
        }

        let mut user_rating = UserRating {
            name: user.username.clone(),
            user_id: user.id,
            department: user.department.clone(),
            position: 0,
            score_sum: extra_point,
            sum_win_exact: 0,
            sum_score_diff: 0,
            sum_team: 0,
            extra_point,
            tips: Vec::new(),
        };
        let user_tips = tips_by_user.get(&user.id).unwrap_or(&no_tips);

        for game in &games {
            let mut match_info = MatchInfo {
                match_id: game.id.to_string(),
                user: user.username.clone(),
                user_id: user.id,
                score: 0,
                team1: serde_json::from_str(&game.home_team).unwrap_or_default(),
                team2: serde_json::from_str(&game.away_team).unwrap_or_default(),
                tip_home: None,
                tip_away: None,
                score_home: Some(game.home_score),
                score_away: Some(game.away_score),
                date: game.date,
            };

            if let Some(tip) = user_tips.get(&game.id) {
                match_info.tip_home = Some(tip.score_home);
                match_info.tip_away = Some(tip.score_away);

                calculate_score(&mut match_info);

                user_rating.score_sum += match_info.score;
                if match_info.score == ScoreConfig::WIN_EXACT {
                    user_rating.sum_win_exact += 1;
                } else if match_info.score == ScoreConfig::WIN_SCORE_DIFF {
                    user_rating.sum_score_diff += 1;
                } else if match_info.score == ScoreConfig::WIN_TEAM {
                    user_rating.sum_team += 1;
                }
            }

            user_rating.tips.push(match_info);
        }
        user_rating_list.push(user_rating);
    }

    Ok(user_rating_list)
}

pub fn calculate_positions(user_rating_list: &mut Vec<UserRating>, clear_tips: bool) {
    user_rating_list.sort_by_key(|u| std::cmp::Reverse(u.score_sum));

    let mut position = 0;
    let mut last_point = -1;
    let mut position_for_frontend = 0;

    for user_rating in user_rating_list {
        position += 1;
        if user_rating.score_sum != last_point {
            position_for_frontend = position;
        }

        user_rating.position = position_for_frontend;

        last_point = user_rating.score_sum;

        if clear_tips {
            user_rating.tips.clear();
        }
    }
}

fn calculate_score(match_info: &mut MatchInfo) {
    if let (Some(score_home), Some(score_away), Some(tip_home), Some(tip_away)) = (
        match_info.score_home,
        match_info.score_away,
        match_info.tip_home,
        match_info.tip_away,
    ) {
        if (score_home > score_away && tip_home > tip_away)
            || (score_home < score_away && tip_home < tip_away)
        {
            match_info.score = ScoreConfig::WIN_TEAM;
        }

        if score_home - score_away == tip_home - tip_away {
            if score_home == score_away {
                match_info.score = ScoreConfig::WIN_TEAM;
            } else {
                match_info.score = ScoreConfig::WIN_SCORE_DIFF;
            }
        }

        if score_home == tip_home && score_away == tip_away {
            match_info.score = ScoreConfig::WIN_EXACT;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rstest::rstest;

    #[test]
    fn test_calculate_positions() {
        let mut user_rating_list = vec![
            UserRating {
                name: "jahnedoe".to_string(),
                score_sum: 2,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
            UserRating {
                name: "ninja".to_string(),
                score_sum: 5,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
            UserRating {
                name: "babo".to_string(),
                score_sum: 10,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
            UserRating {
                name: "abdul".to_string(),
                score_sum: 9,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
            UserRating {
                name: "rockstar".to_string(),
                score_sum: 5,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
            UserRating {
                name: "theBest".to_string(),
                score_sum: 8,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
            UserRating {
                name: "johndoe".to_string(),
                score_sum: 9,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
        ];

        calculate_positions(&mut user_rating_list, true);

        assert_eq!(user_rating_list[0].position, 1);
        assert_eq!(user_rating_list[0].name, "babo");
        assert_eq!(user_rating_list[1].position, 2);
        assert_eq!(user_rating_list[1].name, "abdul");
        assert_eq!(user_rating_list[2].position, 2);
        assert_eq!(user_rating_list[2].name, "johndoe");
        assert_eq!(user_rating_list[3].position, 4);
        assert_eq!(user_rating_list[3].name, "theBest");
        assert_eq!(user_rating_list[4].position, 5);
        assert_eq!(user_rating_list[4].name, "ninja");
        assert_eq!(user_rating_list[5].position, 5);
        assert_eq!(user_rating_list[5].name, "rockstar");
        assert_eq!(user_rating_list[6].position, 7);
        assert_eq!(user_rating_list[6].name, "jahnedoe");
    }

    #[test]
    fn test_calculate_positions_when_two_first_place() {
        let mut user_rating_list = vec![
            UserRating {
                name: "jahnedoe".to_string(),
                score_sum: 8,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
            UserRating {
                name: "ninja".to_string(),
                score_sum: 10,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
            UserRating {
                name: "babo".to_string(),
                score_sum: 10,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
            UserRating {
                name: "abdul".to_string(),
                score_sum: 9,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
            UserRating {
                name: "rockstar".to_string(),
                score_sum: 5,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
            UserRating {
                name: "theBest".to_string(),
                score_sum: 5,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
            UserRating {
                name: "johndoe".to_string(),
                score_sum: 9,
                user_id: 1,
                department: "test".to_string(),
                position: 0,
                sum_win_exact: 0,
                sum_score_diff: 0,
                sum_team: 0,
                extra_point: 0,
                tips: Vec::new(),
            },
        ];

        calculate_positions(&mut user_rating_list, true);

        assert_eq!(user_rating_list[0].position, 1);
        assert_eq!(user_rating_list[0].name, "ninja");
        assert_eq!(user_rating_list[1].position, 1);
        assert_eq!(user_rating_list[1].name, "babo");
        assert_eq!(user_rating_list[2].position, 3);
        assert_eq!(user_rating_list[2].name, "abdul");
        assert_eq!(user_rating_list[3].position, 3);
        assert_eq!(user_rating_list[3].name, "johndoe");
        assert_eq!(user_rating_list[4].position, 5);
        assert_eq!(user_rating_list[4].name, "jahnedoe");
        assert_eq!(user_rating_list[5].position, 6);
        assert_eq!(user_rating_list[5].name, "rockstar");
        assert_eq!(user_rating_list[6].position, 6);
        assert_eq!(user_rating_list[6].name, "theBest");
    }

    #[rstest]
    #[case(1, 2, 1, 2, ScoreConfig::WIN_EXACT)]
    #[case(2, 1, 2, 1, ScoreConfig::WIN_EXACT)]
    #[case(2, 0, 2, 0, ScoreConfig::WIN_EXACT)]
    #[case(0, 2, 0, 2, ScoreConfig::WIN_EXACT)]
    #[case(2, 2, 2, 2, ScoreConfig::WIN_EXACT)]
    #[case(2, 1, 0, 1, ScoreConfig::NO_WIN_TEAM)]
    #[case(1, 3, 3, 2, ScoreConfig::NO_WIN_TEAM)]
    #[case(0, 0, 2, 0, ScoreConfig::NO_WIN_TEAM)]
    #[case(0, 1, 0, 0, ScoreConfig::NO_WIN_TEAM)]
    #[case(1, 3, 2, 4, ScoreConfig::WIN_SCORE_DIFF)]
    #[case(4, 2, 3, 1, ScoreConfig::WIN_SCORE_DIFF)]
    #[case(1, 0, 2, 1, ScoreConfig::WIN_SCORE_DIFF)]
    #[case(1, 2, 0, 1, ScoreConfig::WIN_SCORE_DIFF)]
    #[case(3, 3, 0, 0, ScoreConfig::WIN_TEAM)]
    #[case(3, 3, 4, 4, ScoreConfig::WIN_TEAM)]
    #[case(1, 3, 1, 2, ScoreConfig::WIN_TEAM)]
    #[case(2, 1, 3, 1, ScoreConfig::WIN_TEAM)]
    #[case(1, 0, 2, 0, ScoreConfig::WIN_TEAM)]
    #[case(0, 5, 0, 2, ScoreConfig::WIN_TEAM)]
    #[case(2, 3, 2, 5, ScoreConfig::WIN_TEAM)]
    fn test_calculate_score(
        #[case] score_home: i32,
        #[case] score_away: i32,
        #[case] tip_home: i32,
        #[case] tip_away: i32,
        #[case] expected: i32,
    ) {
        let mut match_info = MatchInfo {
            match_id: "1".to_string(),
            user: "user".to_string(),
            user_id: 1,
            score: 0,
            team1: Team {
                name: String::from("Team1"),
                tla: String::from("te1"),
            },
            team2: Team {
                name: String::from("Team2"),
                tla: String::from("te2"),
            },
            tip_home: Some(tip_home),
            tip_away: Some(tip_away),
            score_home: Some(score_home),
            score_away: Some(score_away),
            date: 1718048296,
        };

        calculate_score(&mut match_info);

        assert_eq!(
            match_info.score, expected,
            "Error: score_home: {}, score_away: {}, tip_home: {}, tip_away: {}",
            score_home, score_away, tip_home, tip_away
        );
    }

    #[test]
    fn test_team_deserializes_null_and_missing_fields_without_panic() {
        let null_fields: Team = serde_json::from_str(r#"{"name":null,"tla":null}"#).unwrap();
        assert_eq!(null_fields.name, "");
        assert_eq!(null_fields.tla, "");

        let missing_fields: Team = serde_json::from_str("{}").unwrap();
        assert_eq!(missing_fields.name, "");
        assert_eq!(missing_fields.tla, "");

        let partial: Team = serde_json::from_str(r#"{"name":"Germany"}"#).unwrap();
        assert_eq!(partial.name, "Germany");
        assert_eq!(partial.tla, "");
    }

    #[test]
    fn test_get_user_rating_does_not_panic_on_malformed_team_json() {
        let games = vec![
            Game {
                id: 1,
                home_team: r#"{"name":null,"tla":null}"#.to_string(),
                away_team: "not valid json".to_string(),
                home_score: 1,
                away_score: 0,
                date: 1718048296,
            },
            Game {
                id: 2,
                home_team: "{}".to_string(),
                away_team: r#"{"name":"France","tla":"FRA"}"#.to_string(),
                home_score: 2,
                away_score: 2,
                date: 1718048297,
            },
        ];

        let users = vec![User {
            id: 999,
            username: "TestUser".to_string(),
            department: "test".to_string(),
            winner: String::new(),
            secret_winner: String::new(),
        }];

        std::env::set_var("MODE", "test");
        let conn = crate::db::establish_connection().unwrap();
        let result =
            get_user_rating(games, users, &conn, "").expect("rating computation must not error");

        assert_eq!(result.len(), 1);
        let tips = &result[0].tips;
        assert_eq!(tips.len(), 2);
        assert_eq!(tips[0].team1.name, "");
        assert_eq!(tips[0].team1.tla, "");
        assert_eq!(tips[0].team2.name, "");
        assert_eq!(tips[0].team2.tla, "");
        assert_eq!(tips[1].team2.name, "France");
        assert_eq!(tips[1].team2.tla, "FRA");
    }

    #[rstest]
    #[case(Some(0), Some(1), None, None, ScoreConfig::NO_WIN_TEAM)]
    #[case(Some(0), Some(0), None, None, ScoreConfig::NO_WIN_TEAM)]
    #[case(Some(1), Some(0), None, None, ScoreConfig::NO_WIN_TEAM)]
    #[case(None, None, Some(1), Some(0), ScoreConfig::NO_WIN_TEAM)]
    #[case(None, None, Some(0), Some(0), ScoreConfig::NO_WIN_TEAM)]
    #[case(None, None, Some(0), Some(1), ScoreConfig::NO_WIN_TEAM)]
    fn test_calculate_score_with_none(
        #[case] score_home: Option<i32>,
        #[case] score_away: Option<i32>,
        #[case] tip_home: Option<i32>,
        #[case] tip_away: Option<i32>,
        #[case] expected: i32,
    ) {
        let mut match_info = MatchInfo {
            match_id: "1".to_string(),
            user: "user".to_string(),
            user_id: 1,
            score: 0,
            team1: Team {
                name: String::from("Team1"),
                tla: String::from("te1"),
            },
            team2: Team {
                name: String::from("Team2"),
                tla: String::from("te2"),
            },
            tip_home,
            tip_away,
            score_home,
            score_away,
            date: 1718048296,
        };

        calculate_score(&mut match_info);

        assert_eq!(match_info.score, expected);
    }
}
