import subprocess
from pathlib import Path

FEATURES = ["vulkan"]

ROOT_DIR = Path(__file__).resolve().parent.parent
EXAMPLES_DIR = ROOT_DIR / "examples"

if __name__ == "__main__":
    examples = sorted(path.stem for path in EXAMPLES_DIR.glob("*.rs"))

    for feature in FEATURES:
        feature_args = ["--no-default-features", "--features", feature]

        subprocess.run(
            ["cargo", "build", "--examples", *feature_args],
            cwd=ROOT_DIR,
            check=True,
        )

        for example in examples:
            subprocess.run(
                ["cargo", "run", "--example", example, *feature_args],
                cwd=ROOT_DIR,
                check=True,
            )
