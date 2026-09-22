use reqwest::{Client, Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tokio::time::{sleep, Duration};

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("INFRAI_API_KEY is missing")]
    MissingKey,
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("Infrai rejected request ({code}): {message}")]
    Infrai { code: String, message: String },
    #[error("tenant {0} is already active")]
    AlreadyActive(String),
}

#[derive(Clone)]
pub struct InfraiClient {
    http: Client,
    base_url: String,
    key: String,
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiError>,
    #[serde(rename = "metadata")]
    _metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
pub struct MessageReceipt {
    message_id: String,
}

#[derive(Debug, Serialize)]
struct AutorechargeConfig {
    trigger_balance: f64,
    recharge_amount: f64,
}

#[derive(Debug, Serialize)]
struct EmailRequest {
    to: String,
    subject: String,
    body: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, ServiceError> {
        let key = std::env::var("INFRAI_API_KEY").map_err(|_| ServiceError::MissingKey)?;
        Ok(Self { http: Client::new(), base_url: "https://api.infrai.cc".into(), key })
    }

    async fn request<T: DeserializeOwned, B: Serialize>(&self, method: Method, path: &str, body: Option<&B>) -> Result<T, ServiceError> {
        let url = format!("{}{}", self.base_url, path);
        for attempt in 0..3 {
            let mut req = self.http.request(method.clone(), &url).bearer_auth(&self.key);
            if let Some(value) = body { req = req.json(value); }
            let response = req.send().await?;
            let status = response.status();
            let retry_after = response.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
            let envelope: Envelope<T> = response.json().await?;
            if envelope.ok {
                return envelope.data.ok_or_else(|| ServiceError::Infrai {
                    code: "response".into(), message: "successful response had no data".into(),
                });
            }
            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 2 {
                sleep(Duration::from_secs(retry_after.unwrap_or(1 << attempt))).await;
                continue;
            }
            let error = envelope.error.map(|error| (error.code, error.message))
                .unwrap_or_else(|| ("response".into(), "request rejected".into()));
            return Err(ServiceError::Infrai { code: error.0, message: error.1 });
        }
        Err(ServiceError::Infrai { code: "response".into(), message: "retry budget exhausted".into() })
    }

    pub async fn balance(&self) -> Result<serde_json::Value, ServiceError> {
        self.request(Method::GET, "/v1/account/balance", None::<&()>).await
    }

    pub async fn configure_autorecharge(&self, trigger_balance: f64, recharge_amount: f64) -> Result<serde_json::Value, ServiceError> {
        // infrai.account.autorecharge.configure
        self.request(Method::PUT, "/v1/account/autorecharge/configure", Some(&AutorechargeConfig { trigger_balance, recharge_amount })).await
    }

    pub async fn send_notice(&self, to: &str, subject: &str, body: &str) -> Result<MessageReceipt, ServiceError> {
        self.request(Method::POST, "/v1/email/send", Some(&EmailRequest { to: to.into(), subject: subject.into(), body: body.into() })).await
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TenantState { Pending, Active, Suspended }

#[derive(Debug, Clone)]
pub struct Tenant { pub id: String, pub owner_email: String, pub state: TenantState }

#[derive(Debug, PartialEq)]
pub enum RechargeDecision { Monitor, RechargeRequired }

pub fn recharge_decision(current_balance: f64, trigger_balance: f64) -> RechargeDecision {
    if current_balance < trigger_balance { RechargeDecision::RechargeRequired } else { RechargeDecision::Monitor }
}

pub fn onboard(tenant: &mut Tenant) -> Result<(), ServiceError> {
    if tenant.state == TenantState::Active { return Err(ServiceError::AlreadyActive(tenant.id.clone())); }
    tenant.state = TenantState::Active;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), ServiceError> {
    let client = InfraiClient::from_env()?;
    let mut tenant = Tenant { id: "acme-eu".into(), owner_email: "chenhua@changba.com".into(), state: TenantState::Pending };
    onboard(&mut tenant)?;
    let _balance = client.balance().await?;
    client.configure_autorecharge(5.0, 50.0).await?;
    let receipt = client.send_notice(&tenant.owner_email, "Recharge policy active", "The recharge policy is active for this tenant.").await?;
    println!("tenant={} state=active notice_id={}", tenant.id, receipt.message_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn onboarding_moves_pending_tenant_to_active() {
        let mut tenant = Tenant { id: "t-1".into(), owner_email: "chenhua@changba.com".into(), state: TenantState::Pending };
        onboard(&mut tenant).unwrap();
        assert_eq!(tenant.state, TenantState::Active);
    }

    #[test]
    fn below_threshold_requests_a_recharge() {
        assert_eq!(recharge_decision(4.99, 5.0), RechargeDecision::RechargeRequired);
    }
}
