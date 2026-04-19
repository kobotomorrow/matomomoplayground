package main

import (
	"context"
	"database/sql"
	"fmt"
	"os"

	"github.com/aws/aws-lambda-go/lambda"
	_ "github.com/duckdb/duckdb-go/v2"
)

const outputKey = "curated/running_distance/running_distance.parquet"

type response struct {
	OutputPath string `json:"output_path"`
}

func main() {
	lambda.Start(handle)
}

func handle(ctx context.Context) (response, error) {
	bucket := os.Getenv("CURATED_S3_BUCKET")
	if bucket == "" {
		return response{}, fmt.Errorf("CURATED_S3_BUCKET is required")
	}

	region := os.Getenv("AWS_REGION")
	if region == "" {
		region = "ap-northeast-1"
	}

	db, err := sql.Open("duckdb", "")
	if err != nil {
		return response{}, err
	}
	defer db.Close()

	if _, err := db.Exec(initSQL(region)); err != nil {
		return response{}, err
	}

	outputPath := fmt.Sprintf("s3://%s/%s", bucket, outputKey)
	if _, err := db.Exec(buildCuratedActivityDistanceSQL(bucket)); err != nil {
		return response{}, err
	}

	return response{OutputPath: outputPath}, nil
}

func initSQL(region string) string {
	return fmt.Sprintf(`
SET home_directory='/tmp';
SET extension_directory='/tmp/duckdb/extensions';

INSTALL httpfs;
LOAD httpfs;

CREATE OR REPLACE SECRET matomomoplayground_s3 (
    TYPE s3,
    PROVIDER credential_chain,
    REGION '%s'
);
`, region)
}

func buildCuratedActivityDistanceSQL(bucket string) string {
	return fmt.Sprintf(`
COPY (
    WITH fitbit_activities AS (
        SELECT
            'fitbit' AS source,
            CAST(year AS BIGINT) AS year,
            CAST(month AS BIGINT) AS month,
            CAST(day AS BIGINT) AS day,
            start_date AS activity_date,
            CAST(start_time AS TIMESTAMP) AS start_time,
            distance
        FROM read_parquet(
            's3://%s/data/fitbit/activities/year=*/month=*/day=*/activities.parquet',
            hive_partitioning = true
        )
        WHERE start_date >= DATE '2025-03-01'
          AND start_date <= DATE '2026-03-13'
    ),
    polar_exercises AS (
        SELECT
            'polar' AS source,
            CAST(year AS BIGINT) AS year,
            CAST(month AS BIGINT) AS month,
            CAST(day AS BIGINT) AS day,
            start_date AS activity_date,
            CAST(start_time AS TIMESTAMP) AS start_time,
            distance
        FROM read_parquet(
            's3://%s/data/polar/exercises/year=*/month=*/day=*/exercises.parquet',
            hive_partitioning = true
        )
        WHERE start_date >= DATE '2026-03-13'
    )
    SELECT
        source,
        year,
        month,
        day,
        activity_date,
        start_time,
        distance
    FROM fitbit_activities

    UNION ALL

    SELECT
        source,
        year,
        month,
        day,
        activity_date,
        start_time,
        distance
    FROM polar_exercises
    ORDER BY activity_date, start_time, source
)
TO 's3://%s/%s'
(FORMAT PARQUET)
`, bucket, bucket, bucket, outputKey)
}
