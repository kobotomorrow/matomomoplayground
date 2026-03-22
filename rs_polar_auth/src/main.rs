use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use std::error::Error;
use std::io;
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. 環境変数からPolarのクライアント情報を読み込む
    let client_id = ClientId::new(
        std::env::var("POLAR_CLIENT_ID")
            .map_err(|_| "POLAR_CLIENT_ID 環境変数が設定されていません")?
    );
    let client_secret = ClientSecret::new(
        std::env::var("POLAR_CLIENT_SECRET")
            .map_err(|_| "POLAR_CLIENT_SECRET 環境変数が設定されていません")?
    );

    let auth_url = AuthUrl::new("https://flow.polar.com/oauth2/authorization".to_string())?;
    let token_url = TokenUrl::new("https://polarremote.com/v2/oauth2/token".to_string())?;
    let redirect_url = RedirectUrl::new("http://localhost:8080".to_string())?;

    let client = BasicClient::new(client_id)
        .set_client_secret(client_secret)
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url);

    // 2. PKCE の生成
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // 3. 認可URLの生成
    let (auth_url, _csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("accesslink.read_all".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    println!("1. ブラウザでこのURLを開いてください:\n{}\n", auth_url);

    // 4. 認可コードの入力
    println!("2. リダイレクトされたURLの 'code=' の値をここに貼り付けてください:");
    let mut code_str = String::new();
    io::stdin().read_line(&mut code_str)?;
    let code = AuthorizationCode::new(code_str.trim().to_string());

    // 5. トークンの交換
    let token_result = client
        .exchange_code(code)
        .set_pkce_verifier(pkce_verifier)
        .request_async(&reqwest::Client::new())
        .await?;

    // 6. トークンをホームディレクトリに保存（有効期限付き）
    let home = std::env::var("HOME")
        .map_err(|_| "HOME環境変数が設定されていません")?;
    let rs_polar_dir = std::path::PathBuf::from(home).join(".config/rs_polar");
    std::fs::create_dir_all(&rs_polar_dir)?;
    let tokens_path = rs_polar_dir.join("tokens.json");

    // 有効期限を計算
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let tokens = serde_json::json!({
        "access_token": token_result.access_token().secret(),
        "created_at": now,
    });

    fs::write(&tokens_path, serde_json::to_string_pretty(&tokens)?)?;
    println!("\n✓ トークンを {} に保存しました", tokens_path.display());

    // 7. Polarユーザー登録API呼び出し
    // let member_id = std::env::var("POLAR_MEMBER_ID")
    //     .map_err(|_| "POLAR_MEMBER_ID 環境変数が設定されていません")?;
    // let register_body = serde_json::json!({
    //     "member-id": member_id
    // });
    // let client = reqwest::Client::new();
    // let res = client.post("https://www.polaraccesslink.com/v3/users")
    //     .bearer_auth(token_result.access_token().secret())
    //     .json(&register_body)
    //     .send()
    //     .await?;

    // if res.status().is_success() {
    //     let text = res.text().await?;
    //     println!("\n✓ ユーザー登録成功: {}", text);
    // } else {
    //        let status = res.status();
    //        let err_text = res.text().await?;
    //        println!("\n⚠ ユーザー登録失敗: {} {}", status, err_text);
    // }

    Ok(())
}
