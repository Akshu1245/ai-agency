use axum::{
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr};
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};

#[derive(Debug, Deserialize)]
struct ContactRequest {
    name: String,
    email: String,
    need: Option<String>,
    budget: Option<String>,
    details: Option<String>,
}

#[derive(Debug, Serialize)]
struct ContactResponse {
    ok: bool,
    delivered_via: Vec<String>,
    message: String,
}

async fn health() -> &'static str {
    "ok"
}

async fn handle_contact(Json(payload): Json<ContactRequest>) -> impl IntoResponse {
    let name = payload.name.trim().to_string();
    let email = payload.email.trim().to_string();
    let need = payload.need.unwrap_or_else(|| "Website".to_string());
    let budget = payload.budget.unwrap_or_else(|| "Not shared".to_string());
    let details = payload.details.unwrap_or_else(|| "Not shared".to_string());

    if name.is_empty() || !email.contains('@') || !email.contains('.') {
        return (
            StatusCode::BAD_REQUEST,
            Json(ContactResponse {
                ok: false,
                delivered_via: vec![],
                message: "Please provide a valid name and work email.".to_string(),
            }),
        );
    }

    let client = reqwest::Client::new();
    let mut delivered_via = Vec::new();

    if let (Ok(api_key), Ok(to_email)) = (env::var("RESEND_API_KEY"), env::var("CONTACT_TO_EMAIL")) {
        let from_email = env::var("CONTACT_FROM_EMAIL")
            .unwrap_or_else(|_| "Rashi Agency <onboarding@resend.dev>".to_string());

        let email_body = serde_json::json!({
            "from": from_email,
            "to": [to_email],
            "reply_to": email,
            "subject": format!("New Project Brief: {} — {}", need, name),
            "text": format!(
                "New Project Brief for Rashi Agency\nName: {}\nWork Email: {}\nNeed: {}\nBudget: {}\n\nDetails:\n{}",
                name, email, need, budget, details
            )
        });

        match client
            .post("https://api.resend.com/emails")
            .bearer_auth(api_key)
            .json(&email_body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => delivered_via.push("email".to_string()),
            Ok(resp) => {
                eprintln!("Resend API error: {}", resp.status());
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(ContactResponse {
                        ok: false,
                        delivered_via,
                        message: "Email delivery service returned an error.".to_string(),
                    }),
                );
            }
            Err(err) => {
                eprintln!("Resend request failed: {}", err);
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(ContactResponse {
                        ok: false,
                        delivered_via,
                        message: "Failed to reach email delivery service.".to_string(),
                    }),
                );
            }
        }
    }

    if let Ok(webhook_url) = env::var("WEBHOOK_URL") {
        let hook_payload = serde_json::json!({
            "name": name,
            "email": email,
            "need": need,
            "budget": budget,
            "details": details
        });
        if let Ok(resp) = client.post(&webhook_url).json(&hook_payload).send().await {
            if resp.status().is_success() {
                delivered_via.push("webhook".to_string());
            }
        }
    }

    if delivered_via.is_empty() {
        delivered_via.push("stdout-log".to_string());
        println!(
            "PROJECT_BRIEF_SUBMISSION name={:?} email={:?} need={:?} budget={:?}",
            name, email, need, budget
        );
    }

    let message = if delivered_via.iter().any(|d| d == "email") {
        "Brief sent! We will reply to your work email shortly.".to_string()
    } else {
        "Brief received and logged. Set RESEND_API_KEY & CONTACT_TO_EMAIL to route to Gmail/inbox.".to_string()
    };

    (
        StatusCode::OK,
        Json(ContactResponse {
            ok: true,
            delivered_via,
            message,
        }),
    )
}

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/contact", post(handle_contact))
        .fallback_service(ServeDir::new(".."))
        .layer(cors);

    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Rashi Agency Rust server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
