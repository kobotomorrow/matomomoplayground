package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/aws/aws-lambda-go/lambda"
	"github.com/aws/aws-sdk-go-v2/config"
	"github.com/aws/aws-sdk-go-v2/service/s3"
)

type result struct {
	key string
	err error
}

type response struct {
	Date    string `json:"date"`
	Message string `json:"message"`
}

func main() {
	lambda.Start(handle)
}

func handle(ctx context.Context) (response, error) {
	bucket := os.Getenv("POLAR_S3_BUCKET")
	if bucket == "" {
		return response{}, fmt.Errorf("POLAR_S3_BUCKET is required")
	}

	target := time.Now().AddDate(0, 0, -1)
	date := target.Format("2006-01-02")
	year, month, day := target.Date()

	cfg, err := config.LoadDefaultConfig(ctx)
	if err != nil {
		return response{}, err
	}
	client := s3.NewFromConfig(cfg)

	results := []result{
		checkFile(ctx, client, bucket, fmt.Sprintf(
			"data/polar/exercises/year=%04d/month=%02d/day=%02d/exercises.json",
			year, month, day,
		)),
		checkFile(ctx, client, bucket, fmt.Sprintf(
			"data/polar/exercises/year=%04d/month=%02d/day=%02d/exercises.parquet",
			year, month, day,
		)),
	}

	lines := []string{fmt.Sprintf("date=%s", date)}
	for _, item := range results {
		if item.err != nil {
			lines = append(lines, fmt.Sprintf("NG: %v", item.err))
			continue
		}
		lines = append(lines, "OK")
	}

	message := strings.Join(lines, "\n")
	if err := notifySlack(message); err != nil {
		return response{}, err
	}

	return response{Date: date, Message: message}, nil
}

func checkFile(ctx context.Context, client *s3.Client, bucket, key string) result {
	output, err := client.HeadObject(ctx, &s3.HeadObjectInput{
		Bucket: &bucket,
		Key:    &key,
	})
	if err != nil {
		return result{key: key, err: fmt.Errorf("head object %s: %w", key, err)}
	}
	if output.ContentLength == nil || *output.ContentLength == 0 {
		return result{key: key, err: fmt.Errorf("file size is zero: %s", key)}
	}
	return result{key: key, err: nil}
}

func notifySlack(text string) error {
	webhookURL := strings.TrimSpace(os.Getenv("SLACK_WEBHOOK_URL"))
	if webhookURL == "" {
		return fmt.Errorf("SLACK_WEBHOOK_URL is not set")
	}

	payload := map[string]string{
		"text": text,
	}
	body, _ := json.Marshal(payload)

	req, _ := http.NewRequest(http.MethodPost, webhookURL, bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")

	resp, _ := http.DefaultClient.Do(req)
	if resp != nil {
		defer resp.Body.Close()
	}
	return nil
}
