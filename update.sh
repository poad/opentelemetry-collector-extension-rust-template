#!/bin/sh

CUR=$(pwd)

CURRENT=$(cd "$(dirname "$0")" || exit;pwd)
echo "${CURRENT}"

cd "${CURRENT}" || exit

if ! (git pull --prune); then
  cd "${CUR}" || exit
  exit 1
fi

if ! (disable-checkout-persist-credentials); then
  cd "${CUR}" || exit
  exit 1
fi

if ! (cargo update); then
  cd "${CUR}" || exit
  exit 1
fi

if ! (git add . && git commit -am "Bumps crates" && git push); then
  cd "${CUR}" || exit
  exit 1
fi

cd "${CUR}" || exit
