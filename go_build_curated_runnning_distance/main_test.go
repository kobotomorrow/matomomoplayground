package main

import (
	"strings"
	"testing"
)

func TestCopyDistanceSummarySQLReportsKilometers(t *testing.T) {
	sql := copyDistanceSummarySQL("test-bucket")

	if count := strings.Count(sql, "ROUND(SUM(distance) / 1000, 2) AS distance"); count != 2 {
		t.Fatalf("summary distance should be converted from meters to kilometers in monthly and total aggregations, got %d conversions in:\n%s", count, sql)
	}
}

func TestCopyCuratedActivityDistanceSQLKeepsRawDistance(t *testing.T) {
	sql := copyCuratedActivityDistanceSQL("test-bucket")

	if strings.Contains(sql, "/ 1000") {
		t.Fatalf("parquet output should keep raw distance values, got conversion in:\n%s", sql)
	}
}
