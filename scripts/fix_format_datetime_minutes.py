"""В ClickHouse formatDateTime: %M — месяц (имя), минуты — %i. Правит сырой JSON дашбордов."""
from pathlib import Path

REPLS = [
    ("'%Y-%m-%d %H:%M:%S'", "'%Y-%m-%d %H:%i:%S'"),
    ("'%m-%d %H:%M'", "'%m-%d %H:%i'"),
    ("'%H:%M:%S'", "'%H:%i:%S'"),
]


def main() -> None:
    root = Path(__file__).resolve().parents[1] / "grafana" / "dashboards"
    for path in sorted(root.glob("*.json")):
        text = path.read_text(encoding="utf-8")
        orig = text
        for a, b in REPLS:
            text = text.replace(a, b)
        if text != orig:
            path.write_text(text, encoding="utf-8")
            print("fixed", path.name)


if __name__ == "__main__":
    main()
