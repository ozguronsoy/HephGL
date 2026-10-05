import argparse
import subprocess
from pathlib import Path

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("shader", type=Path)
    args = parser.parse_args()

    possible_shader_dirs = ["shaders", "../shaders"]
    shaders_dir = None
    for dir in possible_shader_dirs:
        if Path.exists(dir):
            shaders_dir = Path(dir)
    if shaders_dir == None:
        raise NotADirectoryError("Shaders directory not found.")

    shader_path = shaders_dir.joinpath(args.shader)
    if not shader_path.is_file():
        raise FileNotFoundError(f"Shader not found: {shader_path}")
    if not shader_path.suffix:
        raise ValueError("Shader file must have an extension.")
    
    output_path = shader_path.with_name(f"{shader_path.stem}_{shader_path.suffix[1:]}.spv")
    
    subprocess.run(
        ["glslc", str(shader_path), "-o", str(output_path)],
        check=True,
    )