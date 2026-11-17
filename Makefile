# Developer entry points. `make check` must pass before any change is considered done.

PYTHON ?= python3

.PHONY: check test test-rust test-python lint fmt design icons install uninstall android-core android-check android-apk

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

# Regenerate every app icon (Linux SVG, Android layers, Play Store PNG) from
# tools/icon/make_icons.py. Needs Chromium for the PNG.
icons:
	$(PYTHON) -I tools/icon/make_icons.py --png

# Install the release binaries, the desktop entry and the icon. Build first as
# your user (cargo build --release), then: sudo make install PREFIX=/usr/local
PREFIX ?= /usr/local
DESTDIR ?=
RELEASE_DIR ?= target/release
APP_ID = org.meredsp206.Editor
ICON = share/icons/hicolor/scalable/apps/$(APP_ID).svg

install:
	@test -x $(RELEASE_DIR)/meredsp206 -a -x $(RELEASE_DIR)/dsp206 || \
		{ echo "build first: cargo build --release"; exit 1; }
	install -Dm755 $(RELEASE_DIR)/meredsp206 $(DESTDIR)$(PREFIX)/bin/meredsp206
	install -Dm755 $(RELEASE_DIR)/dsp206 $(DESTDIR)$(PREFIX)/bin/dsp206
	install -Dm644 data/$(APP_ID).desktop $(DESTDIR)$(PREFIX)/share/applications/$(APP_ID).desktop
	install -Dm644 data/icons/hicolor/scalable/apps/$(APP_ID).svg $(DESTDIR)$(PREFIX)/$(ICON)
	@! command -v gtk-update-icon-cache >/dev/null || gtk-update-icon-cache -q -t $(DESTDIR)$(PREFIX)/share/icons/hicolor
	@! command -v update-desktop-database >/dev/null || update-desktop-database -q $(DESTDIR)$(PREFIX)/share/applications

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/meredsp206 $(DESTDIR)$(PREFIX)/bin/dsp206
	rm -f $(DESTDIR)$(PREFIX)/share/applications/$(APP_ID).desktop $(DESTDIR)$(PREFIX)/$(ICON)
	@! command -v gtk-update-icon-cache >/dev/null || gtk-update-icon-cache -q -t $(DESTDIR)$(PREFIX)/share/icons/hicolor

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
