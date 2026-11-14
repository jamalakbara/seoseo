pub const URI: &str = "ui://seoseo/evidence.html";
pub const MIME: &str = "text/html;profile=mcp-app";
pub fn meta() -> rmcp::model::MetaObject {
    serde_json::from_value(serde_json::json!({"ui":{"resourceUri":URI}}))
        .expect("static widget metadata")
}
pub const HTML: &str = include_str!("evidence-widget.html");
