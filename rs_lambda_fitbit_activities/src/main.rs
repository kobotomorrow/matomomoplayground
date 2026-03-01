fn get_yesterday_date() -> Result<(String, u32, u32, u32), lambda_runtime::Error> {
    use chrono::Datelike;
    let yesterday = chrono::Local::now() - chrono::Duration::days(1);
    let date = yesterday.format("%Y-%m-%d").to_string();
    let year = yesterday.year() as u32;
    let month = yesterday.month();
    let day = yesterday.day();
    Ok((date, year, month, day))
}

async fn get_token_from_secret_manager(
    config: &aws_config::SdkConfig,
    secret_name: &str,
) -> Result<serde_json::Value, lambda_runtime::Error> {
    let client = aws_sdk_secretsmanager::Client::new(config);
    
    let response = client.get_secret_value().secret_id(secret_name).send().await?;
    
    let secret_string = response.secret_string().ok_or("Secret string not found")?;
    let tokens: serde_json::Value = serde_json::from_str(secret_string)?;
    Ok(tokens)
}

async fn update_token_in_secret_manager(
    config: &aws_config::SdkConfig,
    secret_name: &str,
    tokens: &serde_json::Value,
) -> Result<(), lambda_runtime::Error> {
    let client = aws_sdk_secretsmanager::Client::new(config);
    
    client.update_secret().secret_id(secret_name).secret_string(serde_json::to_string(tokens)?).send().await?;
    
    println!("Secret Managerのトークンを更新しました");
    Ok(())
}

async fn refresh_token(
    config: &aws_config::SdkConfig,
    tokens: &serde_json::Value,
    secret_name: &str,
) -> Result<serde_json::Value, lambda_runtime::Error> {
    let refresh_token_str = tokens["refresh_token"].as_str().ok_or("refresh_token が見つかりません")?;
    
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.fitbit.com/oauth2/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token_str),
        ])
        .send()
        .await?;
    
    if !response.status().is_success() { return Err("トークンリフレッシュ失敗".into()); }
    
    let token_response = response.json::<serde_json::Value>().await?;
    
    let now = chrono::Utc::now().timestamp() as u64;
    let expires_in = token_response["expires_in"].as_u64().unwrap_or(3600);
    let expires_at = now + expires_in;

    let new_tokens = serde_json::json!({
        "access_token": token_response["access_token"].as_str().unwrap_or(""),
        "refresh_token": token_response["refresh_token"].as_str().unwrap_or(refresh_token_str),
        "expires_at": expires_at,
        "created_at": now,
    });
    
    update_token_in_secret_manager(config, secret_name, &new_tokens).await?;
    println!("トークンをリフレッシュしました");
    
    Ok(new_tokens)
}

async fn upload_to_s3(
    config: &aws_config::SdkConfig,
    bucket: &str,
    key: &str,
    content: &str,
) -> Result<(), lambda_runtime::Error> {
    let client = aws_sdk_s3::Client::new(config);
    
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(aws_sdk_s3::primitives::ByteStream::from(content.as_bytes().to_vec()))
        .content_type("application/json")
        .send()
        .await?;
    
    println!("S3へアップロードしました: s3://{}/{}", bucket, key);
    Ok(())
}

async fn handler(_event: lambda_runtime::LambdaEvent<serde_json::Value>) -> Result<serde_json::Value, lambda_runtime::Error> {
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    
    // 設定値（環境変数から取得）
    let secret_name = std::env::var("FITBIT_SECRET_NAME").map_err(|_| "FITBIT_SECRET_NAME 環境変数が設定されていません")?;
    let s3_bucket = std::env::var("FITBIT_S3_BUCKET").map_err(|_| "FITBIT_S3_BUCKET 環境変数が設定されていません")?;
    
    // 1. 前日の日付を取得 (YYYY-MM-DD 形式)
    let (date, year, month, day) = get_yesterday_date()?;
    
    println!("取得対象日付: {}", date);
    
    // 2. Secret Managerからトークンを取得
    let mut tokens = get_token_from_secret_manager(&config, &secret_name).await?;
    println!("Secret Managerからトークンを取得しました");
    
    // 3. トークンの有効期限をチェック
    let now = chrono::Local::now().timestamp() as u64;
    
    let expires_at = tokens["expires_at"].as_u64().ok_or("expires_at が見つかりません")?;
    
    if now >= expires_at {
        println!("アクセストークンが期限切れです。リフレッシュします");
        tokens = refresh_token(&config, &tokens, &secret_name).await?;
    } else {
        println!("✓ アクセストークンは有効です");
    }
    
    let access_token = tokens["access_token"].as_str().ok_or("access_token が見つかりません")?;
    
    // 4. Fitbit APIからアクティビティデータを取得
    let client = reqwest::Client::new();
    let url = format!("https://api.fitbit.com/1/user/-/activities/date/{}.json", date);
    
    println!("Fitbit APIからデータを取得します");
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
    
    // 5. S3に保存
    // 形式: s3://bucket/activities/year=YYYY/month=MM/day=DD/activities.json
    let s3_key = format!(
        "activities/year={}/month={:02}/day={:02}/activities.json",
        year, month, day
    );

    let json_str = serde_json::to_string_pretty(&activities_data)?;
    upload_to_s3(&config, &s3_bucket, &s3_key, &json_str).await?;
    
    println!("データ保存先: s3://{}/{}", s3_bucket, s3_key);
    println!("処理が完了しました");
    
    Ok(serde_json::json!({
        "statusCode": 200,
        "body": format!("処理が完了しました。データ保存先: s3://{}/{}", s3_bucket, s3_key),
    }))
}

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    lambda_runtime::run(lambda_runtime::service_fn(handler)).await
}
