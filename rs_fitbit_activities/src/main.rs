use std::error::Error;
use std::fs;

async fn refresh_token(tokens: &serde_json::Value, tokens_path: &std::path::PathBuf) -> Result<serde_json::Value, Box<dyn Error>> {
    let refresh_token_str = tokens["refresh_token"]
        .as_str()
        .ok_or("refresh_token が見つかりません")?;
    
    let client_id = std::env::var("FITBIT_CLIENT_ID")
        .map_err(|_| "FITBIT_CLIENT_ID 環境変数が設定されていません")?;
    let client_secret = std::env::var("FITBIT_CLIENT_SECRET")
        .map_err(|_| "FITBIT_CLIENT_SECRET 環境変数が設定されていません")?;
    
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.fitbit.com/oauth2/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token_str),
        ])
        .basic_auth(&client_id, Some(&client_secret))
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err("トークンリフレッシュ失敗".into());
    }
    
    let token_response = response.json::<serde_json::Value>().await?;
    
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let expires_in = token_response["expires_in"].as_u64().unwrap_or(3600);
    let expires_at = now + expires_in;
    
    let new_tokens = serde_json::json!({
        "access_token": token_response["access_token"].as_str().unwrap_or(""),
        "refresh_token": token_response["refresh_token"].as_str().unwrap_or(refresh_token_str),
        "expires_at": expires_at,
        "created_at": now,
    });
    
    fs::write(tokens_path, serde_json::to_string_pretty(&new_tokens)?)?;
    println!("✓ トークンをリフレッシュしました");
    
    Ok(new_tokens)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let date_list = vec![
        "2026-03-01"
    ];
    
    // 1. ホームディレクトリから保存されたトークンを読み込む
    let home = std::env::var("HOME")
        .map_err(|_| "HOME環境変数が設定されていません")?;
    let tokens_path = std::path::PathBuf::from(home).join(".config/rs_fitbit/tokens.json");
    
    let tokens_content = fs::read_to_string(&tokens_path)
        .map_err(|_| format!("{} が見つかりません。先に rs_fitbit_auth を実行してください", tokens_path.display()))?;
    
    let mut tokens: serde_json::Value = serde_json::from_str(&tokens_content)?;
    
    // 2. トークンの有効期限をチェック
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    
    let expires_at = tokens["expires_at"]
        .as_u64()
        .ok_or("expires_at が見つかりません")?;
    
    if now >= expires_at {
        println!("⚠️ アクセストークンが期限切れです。リフレッシュします");
        tokens = refresh_token(&tokens, &tokens_path).await?;
    } else {
        println!("✓ アクセストークンは有効です");
    }
    
    let access_token = tokens["access_token"]
        .as_str()
        .ok_or("access_token が見つかりません")?;
    
    println!("✓ トークンを読み込みました");
    
    // 3. データを取得対象の日付で取得
    for date in date_list {

        println!("📅 {}のデータを取得します", date);
        
        // 4. Fitbit APIからアクティビティデータを取得
        let client = reqwest::Client::new();
        let url = format!("https://api.fitbit.com/1/user/-/activities/date/{}.json", date);
        
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(format!("APIエラー: {}", response.status()).into());
        }
        
        let activities_data = response.json::<serde_json::Value>().await?;
        
        println!("Fitbit APIからデータを取得しました");
        
        // 4. データをプロジェクト内のファイルに保存
        let year = date.split('-').next().unwrap_or("");
        let month = date.split('-').nth(1).unwrap_or("");
        let day = date.split('-').nth(2).unwrap_or("");
        let output_dir = std::path::PathBuf::from("./activity_data").join(format!("year={}/month={}/day={}", year, month, day));
        fs::create_dir_all(&output_dir)?;
        let output_file = output_dir.join(format!("activities.json"));
        let json_str = serde_json::to_string_pretty(&activities_data)?;
        fs::write(&output_file, json_str)?;
        
        println!("データを {} に保存しました", output_file.display());
    }
    Ok(())
}