#!/bin/bash


# Note, you may need to edit the generated file
bindgen --dynamic-loading BackfillDylib  ~/.local/include/backfill-0/backfill/api.h --output ./src/backfill/backfill_sys.rs 