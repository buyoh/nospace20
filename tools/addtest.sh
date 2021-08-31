#!/bin/bash

cd `dirname $0`/..

BASE_PATH=resources/test/c

for n in `seq -f "%03.0f" 999`; do
  FILENAME_NS=${BASE_PATH}${n}.ns
  if [[ -e $FILENAME_NS ]]; then
    continue
  fi
  FILENAME_NS_TMPL=${BASE_PATH}000.ns
  FILENAME_CHECK=${BASE_PATH}${n}.check.json
  FILENAME_CHECK_TMPL=${BASE_PATH}000.check.json
  cp $FILENAME_NS_TMPL $FILENAME_NS
  cp $FILENAME_CHECK_TMPL $FILENAME_CHECK
  
  echo "test_ok_coding!(test_ok_coding_c$n, \"c$n\");" >> tests/code_test.rs
  break
done