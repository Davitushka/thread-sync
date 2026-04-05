package server

import "testing"

func TestValidateSeverity(t *testing.T) {
	if err := validateSeverity("high"); err != nil {
		t.Fatal(err)
	}
	if err := validateSeverity("invalid"); err == nil {
		t.Fatal("expected error")
	}
}

func TestValidateStatus(t *testing.T) {
	if err := validateStatus("investigating"); err != nil {
		t.Fatal(err)
	}
	if err := validateStatus("open"); err == nil {
		t.Fatal("expected error")
	}
}
