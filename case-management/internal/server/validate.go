package server

import (
	"errors"
	"slices"
)

func validateSeverity(s string) error {
	if slices.Contains([]string{"low", "medium", "high", "critical"}, s) {
		return nil
	}
	return errors.New("invalid severity")
}

func validateStatus(s string) error {
	if slices.Contains([]string{"new", "triaged", "investigating", "contained", "resolved", "closed"}, s) {
		return nil
	}
	return errors.New("invalid status")
}

func validateResolution(s string) error {
	if slices.Contains([]string{"true_positive", "false_positive", "benign", "informational", "other"}, s) {
		return nil
	}
	return errors.New("invalid resolution")
}
