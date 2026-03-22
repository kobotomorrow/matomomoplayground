use std::error::Error;
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. ホームディレクトリから保存されたトークンを読み込む
    let home = std::env::var("HOME")
        .map_err(|_| "HOME環境変数が設定されていません")?;
    let tokens_path = std::path::PathBuf::from(home).join(".config/rs_polar/tokens.json");
    let tokens_content = fs::read_to_string(&tokens_path)
        .map_err(|_| format!("{} が見つかりません。先に rs_polar_auth を実行してください", tokens_path.display()))?;
    let tokens: serde_json::Value = serde_json::from_str(&tokens_content)?;

    // 2. トークンの有効期限をチェック
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let expires_at = tokens["expires_at"]
        .as_u64()
        .ok_or("expires_at が見つかりません")?;
    if now >= expires_at {
        return Err("アクセストークンが期限切れです。rs_polar_authで再取得してください".into());
    }
    let access_token = tokens["access_token"]
        .as_str()
        .ok_or("access_token が見つかりません")?;
    println!("✓ トークンを読み込みました");

    // 3. Polar APIからexercisesデータを取得
    let client = reqwest::Client::new();
    let url = "https://www.polaraccesslink.com/v3/exercises?zones=true";
    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(format!("APIエラー: {}", response.status()).into());
    }
    let exercises_data = response.json::<serde_json::Value>().await?;
    println!("Polar APIからデータを取得しました");

    // 4. データを日付ごとに分割して保存
    let mut date_map: std::collections::BTreeMap<String, Vec<serde_json::Value>> = std::collections::BTreeMap::new();
    if let Some(arr) = exercises_data.as_array() {
        for item in arr {
            if let Some(start_time) = item.get("start_time").and_then(|v| v.as_str()) {
                if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(start_time, "%Y-%m-%dT%H:%M:%S") {
                    let date = dt.format("%Y-%m-%d").to_string();
                    date_map.entry(date).or_default().push(item.clone());
                }
            }
        }
    }

    for (date, items) in date_map {
        let parts: Vec<&str> = date.split('-').collect();
        if parts.len() == 3 {
            let year = parts[0];
            let month = parts[1];
            let day = parts[2];
            let output_dir = std::path::PathBuf::from("./exercises_data").join(format!("year={}/month={}/day={}", year, month, day));
            fs::create_dir_all(&output_dir)?;
            let output_file = output_dir.join("exercises.json");
            let json_str = serde_json::to_string_pretty(&items)?;
            fs::write(&output_file, json_str)?;
            println!("{} のデータを {} に保存しました", date, output_file.display());
        }
    }

    Ok(())
}
