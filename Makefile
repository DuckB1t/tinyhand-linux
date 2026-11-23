PYTHON ?= python3
VENV ?= .venv
PIP := $(VENV)/bin/pip
PY := $(VENV)/bin/python
SRC := src

.PHONY: venv deps test build install run clean

venv:
	$(PYTHON) -m venv $(VENV)

deps: venv
	$(PIP) install -q --upgrade pip
	$(PIP) install -q -e .

test:
	$(PY) -m unittest discover -s tests -t . -v

build: deps
	$(PIP) install -q build
	$(PY) -m build

install:
	./install.sh

run: deps
	PYTHONPATH=$(SRC) $(PY) -m tinyhand_linux

clean:
	rm -rf build dist *.egg-info $(VENV)
	find . -name __pycache__ -type d -prune -exec rm -rf {} +
