# Import an EDT project into ConfigSave

## Why

Issue #555 removes the intermediate Designer XML directory from whole-project imports.

## What changes

Select an EDT input with the same explicit format and source profile used by convert. Validate the project before opening the database. Pass converted content directly to the existing staging and verification machinery.
