# Jupyter Kernel Setup

## Purpose

This runbook describes how to configure and use a dedicated Jupyter kernel for the Transit Intelligence notebook environment.

The notebook environment is intentionally independent from the project's production Python environments. It is managed by `uv` under:

```text
notebooks/
├── pyproject.toml
├── uv.lock
├── .venv/
└── ...
```

The Jupyter kernel must use this environment directly.

The kernel for Transit Intelligence is:

```text
Name:         transit-intelligence
Display name: Transit Intelligence (Python 3.13)
```

---

## 1. Enter the notebook environment

From the repository root:

```bash
cd notebooks
```

Verify that `uv` is using the notebook environment:

```bash
uv run python --version
uv run which python
```

The Python executable should resolve to:

```text
<repository>/notebooks/.venv/bin/python
```

For example:

```text
/home/user/playground/transit-intelligence/notebooks/.venv/bin/python
```

---

## 2. Verify Jupyter and ipykernel

Run:

```bash
uv run jupyter --version
```

Confirm that Jupyter and `ipykernel` are available.

If they are not installed, add them to the notebook environment:

```bash
uv add jupyter ipykernel
```

Install other notebook dependencies normally through `uv`, for example:

```bash
uv add polars
```

Do not install notebook dependencies into a root-level Python environment.

---

## 3. Register the Transit Intelligence kernel

Register the notebook environment as a user-level Jupyter kernel:

```bash
uv run python -m ipykernel install --user \
  --name transit-intelligence \
  --display-name "Transit Intelligence (Python 3.13)"
```

This creates a kernel specification under:

```text
~/.local/share/jupyter/kernels/transit-intelligence/
```

The kernel specification should point to:

```text
notebooks/.venv/bin/python3
```

Verify the registration:

```bash
uv run jupyter kernelspec list
```

Expected output includes:

```text
transit-intelligence    /home/user/.local/share/jupyter/kernels/transit-intelligence
```

---

## 4. Verify the kernel specification

If there is any doubt about which Python interpreter the kernel uses, inspect:

```bash
cat ~/.local/share/jupyter/kernels/transit-intelligence/kernel.json
```

The `argv` entry must point to the Transit Intelligence notebook environment:

```text
/home/user/playground/transit-intelligence/notebooks/.venv/bin/python3
```

The kernel must not point to another project's Python environment.

For example, the following is a different project and must not be reused:

```text
lakehouse-engineering-lab
```

Each project should have its own notebook environment and kernel.

---

## 5. Connect the notebook to the kernel in VS Code

The repository uses VS Code for notebook editing.

Open the `.ipynb` file in VS Code.

In the upper-right corner of the notebook, select:

```text
Select Kernel
```

Choose:

```text
Python Environments
```

Select the interpreter belonging to:

```text
notebooks/.venv
```

For example:

```text
/home/user/playground/transit-intelligence/notebooks/.venv/bin/python
```

VS Code may display the Python environment rather than the Jupyter kernel's display name. This is acceptable as long as it resolves to the notebook environment.

Do not select the kernel belonging to another project.

---

## 6. Verify the notebook interpreter

The first verification cell in a new notebook should be:

```python
import sys

print(sys.executable)
```

It must resolve to:

```text
<repository>/notebooks/.venv/bin/python
```

Then verify a project dependency:

```python
import polars as pl

print(pl.__version__)
```

This confirms that the notebook is executing against the expected environment.

---

## 7. Do not launch Jupyter Lab unnecessarily

The normal workflow is:

```text
VS Code
    ↓
.ipynb
    ↓
Transit Intelligence kernel
    ↓
notebooks/.venv
```

Running the following in a terminal is not required:

```bash
uv run jupyter lab
```

If Jupyter Lab is required for a specific workflow, it can be launched separately, but it is not part of the normal VS Code notebook workflow.

---

## 8. Updating the notebook environment

When notebook dependencies change, update them through `uv`:

```bash
uv add <package>
```

For example:

```bash
uv add polars
```

This updates:

```text
notebooks/pyproject.toml
notebooks/uv.lock
```

Do not manually install packages with:

```bash
pip install ...
```

The `uv` project is the source of truth for the notebook environment.

After changing dependencies, restart the notebook kernel in VS Code so the running kernel picks up the updated environment.

---

## 9. Re-registering the kernel

The kernel registration points to the project's `.venv`.

If the environment is deleted and recreated, for example:

```bash
rm -rf .venv
uv sync
```

re-register the kernel:

```bash
uv run python -m ipykernel install --user \
  --name transit-intelligence \
  --display-name "Transit Intelligence (Python 3.13)"
```

Then verify:

```bash
uv run jupyter kernelspec list
```

and reconnect the notebook to the Transit Intelligence environment in VS Code.

---

## 10. Troubleshooting

### Kernel does not appear in VS Code

First verify the kernel exists:

```bash
uv run jupyter kernelspec list
```

Then verify the project interpreter:

```bash
uv run which python
```

It should point to:

```text
notebooks/.venv/bin/python
```

If the environment is correct but VS Code does not show it, reload the VS Code window:

```text
Ctrl+Shift+P
→ Developer: Reload Window
```

Then reopen the notebook and select the kernel again.

---

### Kernel exists but points to the wrong Python

Inspect:

```bash
cat ~/.local/share/jupyter/kernels/transit-intelligence/kernel.json
```

If `argv[0]` does not point to:

```text
notebooks/.venv/bin/python3
```

re-register it:

```bash
uv run python -m ipykernel install --user \
  --name transit-intelligence \
  --display-name "Transit Intelligence (Python 3.13)"
```

---

### A kernel from another project appears

Do not use it.

For example:

```text
lakehouse-engineering-lab
```

belongs to another repository and should remain independent.

Select the Python environment under:

```text
transit-intelligence/notebooks/.venv
```

instead.

---

## Expected Architecture

The final relationship should be:

```text
Transit Intelligence
│
└── notebooks/
    │
    ├── pyproject.toml
    ├── uv.lock
    ├── .venv/
    │   └── bin/python
    │
    └── *.ipynb
             │
             ▼
    Transit Intelligence
    Jupyter Kernel
             │
             ▼
    notebooks/.venv/bin/python
```

The notebook environment and kernel are intentionally independent from production domain environments elsewhere in the repository.

## Verification Checklist

Before starting notebook-based analysis:

* [ ] `notebooks/.venv` exists.
* [ ] `uv run python --version` works.
* [ ] `uv run which python` points to `notebooks/.venv`.
* [ ] Jupyter is installed in the notebook environment.
* [ ] `ipykernel` is installed.
* [ ] `transit-intelligence` appears in `jupyter kernelspec list`.
* [ ] The kernel specification points to `notebooks/.venv/bin/python3`.
* [ ] VS Code selects the `notebooks/.venv` Python environment.
* [ ] `sys.executable` inside the notebook points to `notebooks/.venv`.
* [ ] Project dependencies import successfully.
