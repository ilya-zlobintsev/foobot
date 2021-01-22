use reqwest::Client;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationResponse {
    pub src: String,
    pub dest: String,
    pub origin: String,
    pub text: String,
    pub pronunciation: String,
    #[serde(rename = "extra_data")]
    pub extra_data: ExtraData,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraData {}

#[derive(Clone)]
pub struct TranslationHandler {
    client: Client,
}

impl TranslationHandler {
    pub fn new() -> Self {
        TranslationHandler {
            client: Client::new(),
        }
    }

    pub async fn translate(&self, text: &str) -> Result<TranslationResponse, reqwest::Error> {
        Ok(self
            .client
            .get(&format!("http://127.0.0.1:5000/{}", text))
            .send()
            .await?
            .json()
            .await?)
    }
}
