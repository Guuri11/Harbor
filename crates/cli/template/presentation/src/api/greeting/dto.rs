use business::domain::greeting::model::Greeting;
use poem_openapi::Object;

#[derive(Object, Debug)]
#[oai(rename_all = "camelCase")]
pub struct GreetingDto {
    pub name: String,
    pub message: String,
}

impl GreetingDto {
    pub fn from_greeting(greeting: &Greeting) -> Self {
        Self {
            name: greeting.name.clone(),
            message: greeting.message.clone(),
        }
    }
}
