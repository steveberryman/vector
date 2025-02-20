#!/usr/bin/env bash

set -o errexit
set -o pipefail
set -o nounset
#set -o xtrace

__dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOAK_ROOT="${__dir}/.."

display_usage() {
    echo ""
    echo "Usage: $0 CAPTURE_DIR VARIANT IMAGE SOAK_NAME"
}

CAPTURE_DIR="${1}"
VARIANT="${2}"
IMAGE="${3}"
SOAK_NAME="${4}"
WARMUP_GRACE=90
TOTAL_SAMPLES=120

if [  $# -le 1 ]
then
    display_usage
    exit 1
fi

SOAK_CAPTURE_DIR="${CAPTURE_DIR}/${SOAK_NAME}"

pushd "${__dir}"
./boot_minikube.sh
mkdir -p "${SOAK_CAPTURE_DIR}"
minikube mount "${SOAK_CAPTURE_DIR}:/captures" &
minikube cache add "${IMAGE}"
MOUNT_PID=$!
popd

pushd "${SOAK_ROOT}/tests/${SOAK_NAME}/terraform"
terraform init
terraform apply -var "type=${VARIANT}" -var "vector_image=${IMAGE}" -auto-approve -compact-warnings -input=false -no-color -parallelism=20
echo "[${VARIANT}] Captures will be recorded into ${SOAK_CAPTURE_DIR}"
echo "[${VARIANT}] Sleeping for ${WARMUP_GRACE} seconds to allow warm-up"
sleep "${WARMUP_GRACE}"
echo "[${VARIANT}] Recording captures to ${SOAK_CAPTURE_DIR}"
sleep "${TOTAL_SAMPLES}"
kill "${MOUNT_PID}"
popd

pushd "${__dir}"
./shutdown_minikube.sh
popd
