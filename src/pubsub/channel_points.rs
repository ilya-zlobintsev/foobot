#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelPointsRedeem {
    #[serde(rename = "type")]
    pub type_field: String,
    pub data: Data,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub timestamp: String,
    pub redemption: Redemption,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Redemption {
    pub id: String,
    pub user: User,
    #[serde(rename = "channel_id")]
    pub channel_id: String,
    #[serde(rename = "redeemed_at")]
    pub redeemed_at: String,
    pub reward: Reward,
    #[serde(rename = "user_input")]
    pub user_input: Option<String>,
    pub status: String,
    pub cursor: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub login: String,
    #[serde(rename = "display_name")]
    pub display_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reward {
    pub id: String,
    #[serde(rename = "channel_id")]
    pub channel_id: String,
    pub title: String,
    pub prompt: String,
    pub cost: i64,
    #[serde(rename = "is_user_input_required")]
    pub is_user_input_required: bool,
    #[serde(rename = "is_sub_only")]
    pub is_sub_only: bool,
    pub image: ::serde_json::Value,
    #[serde(rename = "default_image")]
    pub default_image: Option<DefaultImage>,
    #[serde(rename = "background_color")]
    pub background_color: String,
    #[serde(rename = "is_enabled")]
    pub is_enabled: bool,
    #[serde(rename = "is_paused")]
    pub is_paused: bool,
    #[serde(rename = "is_in_stock")]
    pub is_in_stock: bool,
    #[serde(rename = "max_per_stream")]
    pub max_per_stream: MaxPerStream,
    #[serde(rename = "should_redemptions_skip_request_queue")]
    pub should_redemptions_skip_request_queue: bool,
    #[serde(rename = "template_id")]
    pub template_id: ::serde_json::Value,
    #[serde(rename = "updated_for_indicator_at")]
    pub updated_for_indicator_at: String,
    #[serde(rename = "max_per_user_per_stream")]
    pub max_per_user_per_stream: MaxPerUserPerStream,
    #[serde(rename = "global_cooldown")]
    pub global_cooldown: GlobalCooldown,
    #[serde(rename = "redemptions_redeemed_current_stream")]
    pub redemptions_redeemed_current_stream: ::serde_json::Value,
    #[serde(rename = "cooldown_expires_at")]
    pub cooldown_expires_at: ::serde_json::Value,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultImage {
    #[serde(rename = "url_1x")]
    pub url1_x: String,
    #[serde(rename = "url_2x")]
    pub url2_x: String,
    #[serde(rename = "url_4x")]
    pub url4_x: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaxPerStream {
    #[serde(rename = "is_enabled")]
    pub is_enabled: bool,
    #[serde(rename = "max_per_stream")]
    pub max_per_stream: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaxPerUserPerStream {
    #[serde(rename = "is_enabled")]
    pub is_enabled: bool,
    #[serde(rename = "max_per_user_per_stream")]
    pub max_per_user_per_stream: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalCooldown {
    #[serde(rename = "is_enabled")]
    pub is_enabled: bool,
    #[serde(rename = "global_cooldown_seconds")]
    pub global_cooldown_seconds: i64,
}
