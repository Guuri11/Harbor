use std::sync::Arc;

use business::{
    application::greeting::get_greeting::GetGreetingUseCaseImpl,
    domain::{
        greeting::use_cases::get_greeting::{GetGreetingParams, GetGreetingUseCaseTrait},
        logger::LoggerTrait,
    },
};
use poem_openapi::{OpenApi, param::Path};

use crate::api::{error::IntoErrorResponse, greeting::{dto::GreetingDto, responses::GetGreetingResponse}, tags::ApiTags};

pub struct GreetingApi {
    pub get_greeting: Arc<GetGreetingUseCaseImpl>,
    pub logger: Arc<dyn LoggerTrait>,
}

#[OpenApi]
impl GreetingApi {
    /// Get a greeting for the given name
    #[oai(path = "/greetings/:name", method = "get", tag = "ApiTags::Greeting")]
    async fn get_greeting(&self, name: Path<String>) -> GetGreetingResponse {
        match self.get_greeting.execute(GetGreetingParams { name: name.0 }).await {
            Ok(greeting) => GetGreetingResponse::Ok(poem_openapi::payload::Json(
                GreetingDto::from_greeting(&greeting),
            )),
            Err(err) => {
                let (status, error) = err.into_error_response();
                self.logger.warn(&format!("get_greeting error: {}", error.0.message));
                GetGreetingResponse::from_status(status, error)
            }
        }
    }
}
