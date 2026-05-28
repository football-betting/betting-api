use crate::service::{calculate_positions, MatchInfo, UserRating};
use crate::{db, service};
use actix_web::{get, web, HttpResponse, Responder, Result as ActixResult};
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize, Deserialize)]
struct StatusResponse {
    status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RatingResponse {
    pub global: Vec<UserRating>,
    pub departments: HashMap<String, Vec<UserRating>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub table: RatingResponse,
    daily_winner: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse {
    pub data: UserRating,
}

#[get("/rating")]
pub async fn rating() -> ActixResult<impl Responder> {
    let mut user_rating_list =
        service::get_user_rating(db::get_past_games().unwrap(), db::get_users().unwrap()).unwrap();

    let cloned_user_rating_list = user_rating_list.clone();
    let mut departments: HashSet<String> = HashSet::new();
    let mut department_ratings: HashMap<String, Vec<UserRating>> = HashMap::new();

    for user_rating in &cloned_user_rating_list {
        departments.insert(user_rating.department.clone());
    }

    for department in departments {
        let mut department_users: Vec<UserRating> = cloned_user_rating_list
            .iter()
            .filter(|user| user.department == department)
            .cloned()
            .collect();

        calculate_positions(&mut department_users, true);
        department_ratings.insert(department, department_users);
    }

    calculate_positions(&mut user_rating_list, true);
    let rating_response = RatingResponse {
        global: user_rating_list,
        departments: department_ratings,
    };

    let response = Response {
        table: rating_response,
        daily_winner: None,
    };

    Ok(HttpResponse::Ok().json(response))
}

#[get("/user/{user_id}")]
pub async fn user_by_id(user_id: web::Path<i32>) -> ActixResult<impl Responder> {
    let mut user_rating_list =
        service::get_user_rating(db::get_past_games().unwrap(), db::get_users().unwrap()).unwrap();

    let user_id = user_id.into_inner();
    calculate_positions(&mut user_rating_list, false);
    let find_user = user_rating_list
        .iter()
        .find(|user| user.user_id == user_id)
        .cloned();

    let response = match find_user {
        Some(mut user) => {
            user.tips.sort_by(|a, b| b.date.cmp(&a.date));
            UserResponse { data: user }
        }
        None => return Ok(HttpResponse::NotFound().body("User not found")),
    };

    Ok(HttpResponse::Ok().json(response))
}

#[get("/game/{game_id}")]
pub async fn get_past_result_by_game_id(game_id: web::Path<String>) -> ActixResult<impl Responder> {
    let user_rating_list =
        service::get_user_rating(db::get_past_games().unwrap(), db::get_users().unwrap()).unwrap();

    let game_id = game_id.into_inner();

    let tips_with_match_id: Vec<&MatchInfo> = user_rating_list
        .iter()
        .flat_map(|user_rating| &user_rating.tips)
        .filter(|tip| tip.match_id == game_id)
        .collect();

    Ok(HttpResponse::Ok().json(tips_with_match_id))
}

#[get("/")]
pub async fn status() -> ActixResult<impl Responder> {
    let response = StatusResponse {
        status: String::from("works"),
    };

    Ok(HttpResponse::Ok().json(response))
}

#[cfg(test)]
mod tests {
    use actix_web::dev::ServiceResponse;
    use actix_web::{test, App};
    use std::env;

    use super::*;

    #[actix_web::test]
    async fn test_get_user_rating() {
        let resp = get_response_by_url("/rating").await;

        assert!(resp.status().is_success());

        let result: Response = test::read_body_json(resp).await;

        assert_eq!(result.table.departments.len(), 2);
        assert_eq!(result.table.global.len(), 7);

        let department = result.table.departments.get("Langenfeld").unwrap();
        assert_eq!(department.len(), 4);
        assert_eq!(department[0].name, "ToniKroos");
        assert_eq!(department[0].department, "Langenfeld");

        let department = result.table.departments.get("London").unwrap();
        assert_eq!(department.len(), 3);
        assert_eq!(department[0].name, "RobbieFowler");
        assert_eq!(department[0].department, "London");

        let global = &result.table.global;
        assert_eq!(global[0].name, "ToniKroos");
        assert_eq!(global[0].department, "Langenfeld");
        assert_eq!(global[0].score_sum, 21);
        assert_eq!(global[0].position, 1);
        assert_eq!(global[0].sum_win_exact, 1);
        assert_eq!(global[0].sum_score_diff, 1);
        assert_eq!(global[0].extra_point, 15);
        assert_eq!(global[0].sum_team, 0);
        assert_eq!(global[0].tips.len(), 0);

        assert_eq!(global[1].name, "JohnDoe");
        assert_eq!(global[1].department, "Langenfeld");
        assert_eq!(global[1].score_sum, 11);
        assert_eq!(global[1].position, 2);
        assert_eq!(global[1].extra_point, 7);

        assert_eq!(global[2].name, "RobbieFowler");
        assert_eq!(global[2].department, "London");
        assert_eq!(global[2].score_sum, 11);
        assert_eq!(global[2].position, 2);

        assert_eq!(global[6].name, "SteveMcManaman");
        assert_eq!(global[6].position, 5);
    }

    #[actix_rt::test]
    async fn test_user_by_id_returns_user_when_exists() {
        let resp = get_response_by_url("/user/2").await;

        assert!(resp.status().is_success());

        let result: UserResponse = test::read_body_json(resp).await;

        assert_eq!(result.data.user_id, 2);
        assert_eq!(result.data.name, "ToniKroos");
        assert_eq!(result.data.department, "Langenfeld");
        assert_eq!(result.data.score_sum, 21);
        assert_eq!(result.data.position, 1);
        assert_eq!(result.data.sum_win_exact, 1);
        assert_eq!(result.data.extra_point, 15);
        assert_eq!(result.data.sum_team, 0);
        assert_eq!(result.data.tips.len(), 2);

        assert_eq!(result.data.tips[0].match_id, "2".to_string());
        assert_eq!(result.data.tips[0].team1.name, "Poland");
        assert_eq!(result.data.tips[0].team2.name, "France");

        assert_eq!(result.data.tips[1].score, 2);
    }

    #[actix_rt::test]
    async fn user_by_id_returns_not_found_when_user_does_not_exist() {
        let resp = get_response_by_url("/user/99999").await;

        assert_eq!(resp.status(), 404);
    }

    #[actix_web::test]
    async fn test_get_past_result_by_game_id() {
        let resp = get_response_by_url("/game/2").await;

        assert!(resp.status().is_success());

        let result: Vec<MatchInfo> = test::read_body_json(resp).await;

        assert!(!result.is_empty());

        assert_eq!(result.len(), 7);
        assert_eq!(result[0].match_id, "2".to_string());
        assert_eq!(result[0].user, "JohnDoe");
        assert_eq!(result[0].score, 0);

        assert_eq!(result[1].match_id, "2".to_string());
        assert_eq!(result[1].user, "ToniKroos");
        assert_eq!(result[1].score, 4);

        assert_eq!(result[2].match_id, "2".to_string());
        assert_eq!(result[2].team1.name, "Poland");
        assert_eq!(result[2].team2.name, "France");
        assert_eq!(result[2].score, 1);
        assert_eq!(result[2].tip_home, Some(2));
        assert_eq!(result[2].tip_away, Some(2));
        assert_eq!(result[2].score_home, Some(1));
        assert_eq!(result[2].score_away, Some(1));
    }

    #[actix_web::test]
    async fn test_status() {
        let resp = get_response_by_url("/").await;

        assert!(resp.status().is_success());

        let result: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(result.get("status").unwrap().as_str().unwrap(), "works");
    }

    async fn get_response_by_url(url: &str) -> ServiceResponse {
        env::set_var("MODE", "test");

        let app = test::init_service(
            App::new()
                .service(get_past_result_by_game_id)
                .service(status)
                .service(user_by_id)
                .service(rating),
        )
        .await;

        let req = test::TestRequest::get().uri(url).to_request();

        let resp = test::call_service(&app, req).await;
        resp
    }
}
