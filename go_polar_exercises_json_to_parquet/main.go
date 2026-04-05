package main

import (
	"context"
	"database/sql"
	"fmt"
	"log"
	"net/url"
	"os"
	"strings"

	"github.com/aws/aws-lambda-go/events"
	"github.com/aws/aws-lambda-go/lambda"
	_ "github.com/duckdb/duckdb-go/v2"
)

func main() {
	lambda.Start(handleS3Event)
}

func handleS3Event(_ context.Context, event events.S3Event) error {
	for _, record := range event.Records {
		if err := handleS3Record(record); err != nil {
			return err
		}
	}

	return nil
}

func handleS3Record(record events.S3EventRecord) error {
	bucketName := record.S3.Bucket.Name
	inputKey, err := url.QueryUnescape(strings.ReplaceAll(record.S3.Object.Key, "+", " "))
	if err != nil {
		return fmt.Errorf("decode s3 object key: %w", err)
	}

	if !strings.HasSuffix(inputKey, ".json") {
		return fmt.Errorf("input key must end with .json: %s", inputKey)
	}
	outputKey := strings.TrimSuffix(inputKey, ".json") + ".parquet"

	db, err := sql.Open("duckdb", "")
	if err != nil {
		return err
	}
	defer db.Close()

	region := os.Getenv("AWS_REGION")
	if region == "" {
		region = "ap-northeast-1"
	}

	initSQL := fmt.Sprintf(`
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

	if _, err := db.Exec(initSQL); err != nil {
		return err
	}

	input := fmt.Sprintf("s3://%s/%s", bucketName, inputKey)
	output := fmt.Sprintf("s3://%s/%s", bucketName, outputKey)

	query := fmt.Sprintf(`
COPY (
	SELECT
		start_time,
		CAST(start_time AS DATE) AS start_date,
		distance,
		duration,
		COALESCE(TRY_CAST(regexp_extract(duration, 'PT(([0-9]+(\\.[0-9]+)?)H)?(([0-9]+(\\.[0-9]+)?)M)?(([0-9]+(\\.[0-9]+)?)S)?', 2) AS DOUBLE), 0) * 3600 +
			COALESCE(TRY_CAST(regexp_extract(duration, 'PT(([0-9]+(\\.[0-9]+)?)H)?(([0-9]+(\\.[0-9]+)?)M)?(([0-9]+(\\.[0-9]+)?)S)?', 5) AS DOUBLE), 0) * 60 +
			COALESCE(TRY_CAST(regexp_extract(duration, 'PT(([0-9]+(\\.[0-9]+)?)H)?(([0-9]+(\\.[0-9]+)?)M)?(([0-9]+(\\.[0-9]+)?)S)?', 8) AS DOUBLE), 0) AS duration_seconds,
		calories,
		fat_percentage,
		carbohydrate_percentage,
		protein_percentage,
		running_index,
		heart_rate,
		list_transform(
			heart_rate_zones,
			lambda z: struct_pack(
				"index" := z."index",
				lower_limit := z.lower_limit,
				upper_limit := z.upper_limit,
				in_zone := z.in_zone,
				in_zone_seconds := COALESCE(TRY_CAST(regexp_extract(z.in_zone, 'PT(([0-9]+(\\.[0-9]+)?)H)?(([0-9]+(\\.[0-9]+)?)M)?(([0-9]+(\\.[0-9]+)?)S)?', 2) AS DOUBLE), 0) * 3600 +
					COALESCE(TRY_CAST(regexp_extract(z.in_zone, 'PT(([0-9]+(\\.[0-9]+)?)H)?(([0-9]+(\\.[0-9]+)?)M)?(([0-9]+(\\.[0-9]+)?)S)?', 5) AS DOUBLE), 0) * 60 +
					COALESCE(TRY_CAST(regexp_extract(z.in_zone, 'PT(([0-9]+(\\.[0-9]+)?)H)?(([0-9]+(\\.[0-9]+)?)M)?(([0-9]+(\\.[0-9]+)?)S)?', 8) AS DOUBLE), 0)
			)
		) AS heart_rate_zones
	FROM read_json(
			'%s',
			columns = {
				start_time: 'VARCHAR',
				duration: 'VARCHAR',
				distance: 'DOUBLE',
				calories: 'BIGINT',
				fat_percentage: 'BIGINT',
				carbohydrate_percentage: 'BIGINT',
				protein_percentage: 'BIGINT',
				running_index: 'BIGINT',
				heart_rate: 'STRUCT(average BIGINT, maximum BIGINT)',
				heart_rate_zones: 'STRUCT("index" BIGINT, lower_limit BIGINT, upper_limit BIGINT, in_zone VARCHAR)[]'
			}
		)
)
TO '%s'
(FORMAT PARQUET)
`, input, output)

	if _, err := db.Exec(query); err != nil {
		return err
	}

	log.Printf("converted s3://%s/%s -> s3://%s/%s", bucketName, inputKey, bucketName, outputKey)
	return nil
}
