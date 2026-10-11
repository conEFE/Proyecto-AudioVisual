"""Carga de configuración con valores por defecto."""

import copy
from pathlib import Path

import yaml

DEFAULT_PATH = Path(__file__).resolve().parent.parent / "config.yaml"


def _merge(base: dict, override: dict) -> dict:
    out = copy.deepcopy(base)
    for key, value in (override or {}).items():
        if isinstance(value, dict) and isinstance(out.get(key), dict):
            out[key] = _merge(out[key], value)
        else:
            out[key] = value
    return out


def load_config(path=None) -> dict:
    """Lee config.yaml del proyecto y, si se indica, otro archivo encima."""
    with open(DEFAULT_PATH, encoding="utf-8") as f:
        cfg = yaml.safe_load(f)
    if path and Path(path).resolve() != DEFAULT_PATH:
        with open(path, encoding="utf-8") as f:
            cfg = _merge(cfg, yaml.safe_load(f))
    return cfg
