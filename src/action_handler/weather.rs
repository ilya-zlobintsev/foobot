use reqwest::Client;
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherResponse {
    pub coord: Coord,
    pub weather: Vec<Weather>,
    pub base: String,
    pub main: Main,
    pub visibility: f64,
    pub wind: Wind,
    pub snow: Option<Snow>,
    pub clouds: Clouds,
    pub dt: f64,
    pub sys: Sys,
    pub timezone: f64,
    pub id: f64,
    pub name: String,
    pub cod: Value,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coord {
    pub lon: f64,
    pub lat: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Weather {
    pub id: f64,
    pub main: String,
    pub description: String,
    pub icon: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Main {
    pub temp: f64,
    #[serde(rename = "feels_like")]
    pub feels_like: f64,
    #[serde(rename = "temp_min")]
    pub temp_min: f64,
    #[serde(rename = "temp_max")]
    pub temp_max: f64,
    pub pressure: f64,
    pub humidity: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wind {
    pub speed: f64,
    pub deg: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snow {
    #[serde(rename = "1h")]
    pub n1_h: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clouds {
    pub all: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sys {
    #[serde(rename = "type")]
    pub type_field: Option<f64>,
    pub id: Option<f64>,
    pub country: Option<String>,
    pub sunrise: f64,
    pub sunset: f64,
}

#[derive(Debug)]
pub enum WeatherError {
    ReqwestError(reqwest::Error),
    ParseError(serde_json::Error),
    InvalidLocation,
}

impl From<reqwest::Error> for WeatherError {
    fn from(err: reqwest::Error) -> Self {
        WeatherError::ReqwestError(err)
    }
}

impl From<serde_json::Error> for WeatherError {
    fn from(err: serde_json::Error) -> Self {
        WeatherError::ParseError(err)
    }
}


#[derive(Clone)]
pub struct WeatherHandler {
    client: Client,
    api_key: String,
}

impl WeatherHandler {
    pub fn new(api_key: String) -> Self {
        WeatherHandler {
            api_key: api_key,
            client: Client::new(),
        }
    }

    pub async fn get_weather(&self, location: String) -> Result<WeatherResponse, WeatherError> {
        let url = format!(
            "https://api.openweathermap.org/data/2.5/weather?q={}&appid={}&units=metric",
            location, self.api_key
        );
        
        let response: Value = self.client.get(&url).send().await?.json().await?;
        
        log::trace!("{:?}", response);
        
        match response["cod"].to_string().as_str() {
            "200" => Ok(serde_json::from_value(response)?),
            "\"404\"" => Err(WeatherError::InvalidLocation),
            code => unimplemented!("code {}", code),
        }
    }
}
