use crate::core::{User, UserConfig, save_config};
use anyhow::{Result, anyhow};
use cliclack::{input, spinner};
use cliclack::{intro, outro};
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub async fn register_run() -> Result<()> {
    let client = Client::new();

    let spinner = spinner();
    spinner.start("Checking server availability...");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Check server availability
    let server_resp = client
        .get("https://api.blaze.sh/v1/blz/health")
        .send()
        .await?;
    if !server_resp.status().is_success() {
        spinner.error("Server is currently unreachable. Please try again later.");
        return Ok(());
    }
    spinner.stop("Server is reachable.\n");

    // TODO: Add checks for existing user registration

    intro("Register yourself 😌")?;

    let user_name: String = input("Enter your user name")
        .placeholder("Skinny hacker")
        .validate(|input: &String| {
            if input.is_empty() {
                Err("Value is required!")
            } else {
                Ok(())
            }
        })
        .interact()?;

    let user_email: String = input("Enter your email address")
        .placeholder("superlonely@nerd.com")
        .validate(|input: &String| email_validate(input))
        .interact()?;

    spinner.start("Registering...");

    let register_req = UserRegisterRequest {
        username: user_name.clone(),
        email: user_email.clone(),
    };

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let resp = client
        .post("https://api.blazedb.online/v1/blz/auth/register")
        .json(&register_req)
        .send()
        .await?;

    let response: UserRegisterResponse = resp.json().await?;

    // Load existing config if any
    let user_config = UserConfig::load_config(&UserConfig::get_default_path()?).await?;

    if response.is_created {
        let user_config = UserConfig {
            user: User {
                username: user_name.clone(),
                email: user_email.clone(),
            },
            server: user_config.server.clone(),
        };

        save_config(UserConfig::get_default_path()?, &user_config).await?;

        spinner.stop("Registered successfully!");
        outro(format!(
            "Welcome aboard 😊, {}!\n You can now verify your email to get your API key",
            user_name
        ))?;
    } else {
        spinner.stop("Oh no..Registration failed!");
        outro(format!("Error: {}", response.error))?;
    }

    Ok(())
}

pub fn email_validate(email: &String) -> Result<()> {
    let email_regex = regex::Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$")?;
    if email_regex.is_match(email) || email.is_empty() {
        Ok(())
    } else {
        Err(anyhow!("Invalid email format"))
    }
}

#[allow(unused)]
/// Request structure for user registration
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct UserRegisterRequest {
    pub username: String,
    pub email: String,
}

#[allow(unused)]
/// Response structure for user registration
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct UserRegisterResponse {
    pub email: String,
    pub is_created: bool,
    pub error: String,
}

#[allow(unused)]
/// Request structure for email verification
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VerifyEmailRequest {
    pub email: String,
}

#[allow(unused)]
/// Response structure if verification code is sent
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VerifyEmailResponse {
    pub is_code_sent: bool,
    pub error: String,
}

#[allow(unused)]
/// Request structure for OTP verification
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VerifyOtpRequest {
    pub email: String,
    pub otp: String,
}

#[allow(unused)]
/// Response structure for OTP verified or not
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VerifyOtpResponse {
    pub is_verified: bool,
    pub message: String,
    pub api_key: Option<String>,
    // pub instance_url: Option<String>,
}
