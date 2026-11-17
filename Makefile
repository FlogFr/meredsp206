# Developer entry points. `make check` must pass before any change is considered done.

PYTHON ?= python3

.PHONY: check test test-rust test-python lint fmt design android-core android-check android-apk

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

# Android app (android/, claude/docs/android.md). Needs a JDK 17+; the targets
# other than android-core also need the Android SDK (ANDROID_HOME or
# android/local.properties).
GRADLE = cd android && ./gradlew --console=plain

android-core:
	$(GRADLE) :core:test

android-check:
	$(GRADLE) :core:test :app:testDebugUnitTest :app:lintDebug

android-apk:
	$(GRADLE) :app:assembleDebug
