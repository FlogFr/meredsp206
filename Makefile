# Developer entry points. `make check` must pass before any change is considered done.

PYTHON ?= python3

.PHONY: check test test-rust test-python lint fmt design

check: lint test

test: test-rust test-python

test-rust:
	cargo test --workspace --all-targets

test-python:
	$(PYTHON) -I -m unittest discover -s tools/tests

lint:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt --all

design:
	design/render.sh
