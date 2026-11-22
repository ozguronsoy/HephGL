import os
import subprocess
from pathlib import Path

RENDERERS = ["vulkan"]

ROOT_DIR = Path(__file__).resolve().parent.parent
EXAMPLES_DIR = ROOT_DIR / "examples"

if __name__ == "__main__":
    examples = sorted(path.stem for path in EXAMPLES_DIR.glob("*.rs"))

    subprocess.run(
        ["cargo", "build", "--examples", "--all-features"],
        cwd=ROOT_DIR,
        check=True,
    )

    for renderer in RENDERERS:
        env = {**os.environ, "HEPHGL_EXAMPLE_RENDERER": renderer}

        for example in examples:
            subprocess.run(
                ["cargo", "run", "--example", example, "--all-features"],
                cwd=ROOT_DIR,
                env=env,
                check=True,
            )
