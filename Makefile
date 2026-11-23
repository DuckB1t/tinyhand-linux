CARGO ?= cargo
PYTHON ?= python3
VENV ?= .venv
PY := $(VENV)/bin/python
MANIFEST := rust/Cargo.toml

.PHONY: deps test test-python build run install clean venv

deps:
	$(CARGO) fetch --manifest-path $(MANIFEST)

test:
	$(CARGO) test --manifest-path $(MANIFEST)

venv:
	$(PYTHON) -m venv $(VENV)
	$(VENV)/bin/pip install -q -e .

test-python: venv
	$(PY) -m unittest discover -s tests -t . -v

build:
	$(CARGO) build --release --manifest-path $(MANIFEST)

run:
	$(CARGO) run --release --manifest-path $(MANIFEST) --bin tinyhand

install:
	./install.sh

clean:
	$(CARGO) clean --manifest-path $(MANIFEST)
	rm -rf build dist *.egg-info $(VENV)
	find . -name __pycache__ -type d -prune -exec rm -rf {} +
