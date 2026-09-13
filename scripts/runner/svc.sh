#!/bin/bash

SVC_NAME="actions.runner.viveky259259-falcon.Home-Server"
SVC_NAME=${SVC_NAME// /_}
SVC_DESCRIPTION="GitHub Actions Runner (viveky259259-falcon.Home-Server)"

user_id=`id -u`

# launchctl should not run as sudo for launch runners
if [ $user_id -eq 0 ]; then
    echo "Must not run with sudo"
    exit 1
fi

SVC_CMD=$1
RUNNER_ROOT=`pwd`

LAUNCH_PATH="${HOME}/Library/LaunchAgents"
PLIST_PATH="${LAUNCH_PATH}/${SVC_NAME}.plist"
TEMPLATE_PATH=$GITHUB_ACTIONS_RUNNER_SERVICE_TEMPLATE
IS_CUSTOM_TEMPLATE=0
if [[ -z $TEMPLATE_PATH ]]; then
    TEMPLATE_PATH=./bin/actions.runner.plist.template
else
    IS_CUSTOM_TEMPLATE=1
fi
TEMP_PATH=./bin/actions.runner.plist.temp
CONFIG_PATH=.service

function failed()
{
   local error=${1:-Undefined error}
   echo "Failed: $error" >&2
   exit 1
}

function service_target()
{
    echo "gui/${user_id}/${SVC_NAME}"
}

# Whether launchd already knows about this job in the user's GUI domain.
# `launchctl load` on an already-loaded job fails with
# "Load failed: 5: Input/output error", so start and stop check this first
# instead of assuming the service is down.
function is_bootstrapped()
{
    launchctl print "`service_target`" >/dev/null 2>&1
}

# launchctl returns before the job has finished coming up or tearing down, so
# poll briefly to make sure the status we print reflects the settled state.
function wait_for_status()
{
    local want=$1
    local attempts=20
    local listed

    while [ $attempts -gt 0 ]; do
        listed=`launchctl list | grep "${SVC_NAME}"`
        if [ "$want" = "started" ] && [ ! -z "$listed" ]; then
            return 0
        fi
        if [ "$want" = "stopped" ] && [ -z "$listed" ]; then
            return 0
        fi
        attempts=$((attempts - 1))
        sleep 0.5
    done

    return 1
}

if [ ! -f "${TEMPLATE_PATH}" ]; then
    if [[ $IS_CUSTOM_TEMPLATE = 0 ]]; then
        failed "Must run from runner root or install is corrupt"
    else
        failed "Service file at '$GITHUB_ACTIONS_RUNNER_SERVICE_TEMPLATE' using GITHUB_ACTIONS_RUNNER_SERVICE_TEMPLATE env variable is not found"
    fi
fi

function install()
{
    echo "Creating launch runner in ${PLIST_PATH}"

    if [ ! -d  "${LAUNCH_PATH}" ]; then
        mkdir ${LAUNCH_PATH}
    fi

    if [ -f "${PLIST_PATH}" ]; then
        failed "error: exists ${PLIST_PATH}"
    fi

    if [ -f "${TEMP_PATH}" ]; then
      rm "${TEMP_PATH}" || failed "failed to delete ${TEMP_PATH}"
    fi

    log_path="${HOME}/Library/Logs/${SVC_NAME}"
    echo "Creating ${log_path}"
    mkdir -p "${log_path}" || failed "failed to create ${log_path}"

    echo Creating ${PLIST_PATH}
    sed "s/{{User}}/${USER:-$SUDO_USER}/g; s/{{SvcName}}/$SVC_NAME/g; s@{{RunnerRoot}}@${RUNNER_ROOT}@g; s@{{UserHome}}@$HOME@g;" "${TEMPLATE_PATH}" > "${TEMP_PATH}" || failed "failed to create replacement temp file"
    mv "${TEMP_PATH}" "${PLIST_PATH}" || failed "failed to copy plist"

    # Since we started with sudo, runsvc.sh will be owned by root. Change this to current login user.
    echo Creating runsvc.sh    
    cp ./bin/runsvc.sh ./runsvc.sh || failed "failed to copy runsvc.sh"
    chmod u+x ./runsvc.sh || failed "failed to set permission for runsvc.sh"

    echo Creating ${CONFIG_PATH}
    echo "${PLIST_PATH}" > ${CONFIG_PATH} || failed "failed to create .Service file"

    echo "svc install complete"
}

function start()
{
    echo "starting ${SVC_NAME}"

    if is_bootstrapped; then
        # Already loaded. Loading a second time is what printed
        # "Load failed: 5: Input/output error"; kickstart only ensures the job
        # is running and is a no-op when it already is.
        launchctl kickstart "`service_target`" >/dev/null 2>&1 || failed "failed to start ${SVC_NAME}"
    else
        launchctl load -w "${PLIST_PATH}" || failed "failed to load ${PLIST_PATH}"
    fi

    wait_for_status started || failed "${SVC_NAME} did not come up"
    status
}

function stop()
{
    echo "stopping ${SVC_NAME}"

    if is_bootstrapped; then
        launchctl unload "${PLIST_PATH}" || failed "failed to unload ${PLIST_PATH}"
    else
        echo "${SVC_NAME} is not loaded"
    fi

    wait_for_status stopped || failed "${SVC_NAME} did not stop"
    status
}

function uninstall()
{
    echo "uninstalling ${SVC_NAME}"
    stop
    rm "${PLIST_PATH}" || failed "failed to delete ${PLIST_PATH}"
    if [ -f "${CONFIG_PATH}" ]; then
      rm "${CONFIG_PATH}" || failed "failed to delete ${CONFIG_PATH}"
    fi
}

function status()
{
    echo "status ${SVC_NAME}:"
    if [ -f "${PLIST_PATH}" ]; then
        echo
        echo "${PLIST_PATH}"
    else
        echo
        echo "not installed"
        echo
        return
    fi

    echo
    status_out=`launchctl list | grep "${SVC_NAME}"`
    if [ ! -z "$status_out" ]; then
        echo Started:
        echo $status_out
        echo
    else
        echo Stopped
        echo
    fi
}

function usage()
{
    echo
    echo Usage:
    echo "./svc.sh [install, start, stop, status, uninstall]"
    echo
}

case $SVC_CMD in
   "install") install;;
   "status") status;;
   "uninstall") uninstall;;
   "start") start;;
   "stop") stop;;
   *) usage;;
esac

exit 0
