// cargo lambda build --release -p rs_lambda_polar_exercises --output-format zip

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
    let secret_name = std::env::var("POLAR_SECRET_NAME").map_err(|_| "POLAR_SECRET_NAME 環境変数が設定されていません")?;
    let s3_bucket = std::env::var("POLAR_S3_BUCKET").map_err(|_| "POLAR_S3_BUCKET 環境変数が設定されていません")?;

    // 1. 前日の日付を取得 (YYYY-MM-DD 形式)
    let (date, year, month, day) = get_yesterday_date()?;
    println!("取得対象日付: {}", date);

    // 2. Secret Managerからトークンを取得
    let tokens = get_token_from_secret_manager(&config, &secret_name).await?;
    println!("Secret Managerからトークンを取得しました");


    // 3. アクセストークンを取得
    let access_token = tokens["access_token"].as_str().ok_or("access_token が見つかりません")?;
    println!("✓ アクセストークンを利用します");

    // 4. Polar APIからexercisesデータを取得
    let client = reqwest::Client::new();
    let url = "https://www.polaraccesslink.com/v3/exercises?zones=true";

    println!("Polar APIからデータを取得します");
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

    // 5. 前日の日付と一致するexercisesのみをフィルタリング
    let filtered: Vec<serde_json::Value> = exercises_data
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|item| {
                    item.get("start_time")
                        .and_then(|v| v.as_str())
                        .and_then(|s| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").ok())
                        .map(|dt| dt.format("%Y-%m-%d").to_string() == date)
                        .unwrap_or(false)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    println!("対象日付 {} に一致するexercises数: {}", date, filtered.len());

    // 6. S3に保存
    // 形式: s3://bucket/data/polar/exercises/year=YYYY/month=MM/day=DD/exercises.json
    let s3_key = format!(
        "data/polar/exercises/year={}/month={:02}/day={:02}/exercises.json",
        year, month, day
    );

    let json_str = serde_json::to_string_pretty(&filtered)?;
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
