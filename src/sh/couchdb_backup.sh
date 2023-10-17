#!/bin/bash
set -o errexit -o nounset -o pipefail

LAST_MONTH=""
CURRENT_DAY_MONTH=""
CURRENT_DAY_WEEK=""
BACKUP_YEAR=""
BACKUP_MONTH=""
BACKUP_DAY=""
PRESENT_YEAR=""
PRESENT_MONTH=""
PRESENT_DAY=""
DATE_PATH_FOR_LAST_MONTH_DB=""
DATE_PATH_FOR_CURRENT_DB=""
COUCHDB_USER=""
COUCHDB_PASS=""
COUCHDB_PORT=""
COUCHDB_URL=""
COUCHDB_ALB_URL=""
COUCHDB_ALB_PORT=""
COUCHDB_ADMINS_INI=""
DAEMON=""
BACKUP_DIR=""
INSTANCE_ID=""
REGION=""
CLIENT=""
SUBSYSTEM=""
HOSTNAME=""
DOMAIN="nga911.com"
S3_BUCKET_URL=""
AWS_CLI_BIN=$(which aws)
CURL_BIN=$(which curl)

# make volume mount as optional flag

debug_on() {
    set -x
}

init_default_vars() {
    HOSTNAME=$(hostname)
    DAEMON="${DAEMON:-couchdb}"
    BACKUP_DAY_MONTH_NUMBER="${BACKUP_DAY_MONTH_NUMBER:-02}"
    BACKUP_DAY_WEEK_NUMBER="${BACKUP_DAY_WEEK_NUMBER:-0}"
    BACKUP_DIR="${BACKUP_DIR:-/tmp/couchdb/backup}"
    RESTORE_DIR="${RESTORE_DIR:-/tmp/couchdb/restore}"
    WORK_DIR="${WORK_DIR:-/tmp/couchdb/work}"
    EXCLUDED_DBS="${EXCLUDED_DBS:-_global_changes|_replicator|_users|acdc|alerts|anonymous_cdrs|faxes|global_provisioner|pending_notifications|system_media|system_schemas|token_auth|webhooks|account%2F|tasks}"
    DOC_LIST="${DOC_LIST:-doc_list.txt}"
    DOCS_AMOUNT="${DOCS_AMOUNT:-1000}"
    LAST_MONTH=$(date '+%Y%m' --date='-1 month')
    CURRENT_DAY_MONTH="$(date '+%d')"
    CURRENT_DAY_WEEK="$(date '+%w')"
    BACKUP_YEAR=$(date '+%Y' --date='-1 month')
    BACKUP_MONTH=$(date '+%m' --date='-1 month')
    BACKUP_DAY=$(date '+%d' --date='-1 month')
    PRESENT_YEAR=$(date '+%Y')
    PRESENT_MONTH=$(date '+%m')
    PRESENT_DAY=$(date '+%d')
}

init_aws_vars() {
    INSTANCE_ID=$(${CURL_BIN} -s http://169.254.169.254/latest/meta-data/instance-id)
    if [ -z "${INSTANCE_ID}" ]; then
        echo "error: cannot get INSTANCE_ID data"
        exit 1
    fi
    REGION=$(${CURL_BIN} -s http://169.254.169.254/latest/dynamic/instance-identity/document | jq -r .region)
    export REGION
    if [ -z "${REGION}" ]; then
        echo "error: cannot get REGION tag"
        exit 1
    fi
    ZONE=$(${CURL_BIN} -s http://169.254.169.254/latest/dynamic/instance-identity/document | jq -r .availabilityZone)
    if [ -z "${ZONE}" ]; then
        echo "error: cannot get ZONE tag"
        exit 1
    fi
    TAGS=$(${AWS_CLI_BIN} ec2 describe-tags --filters "Name=resource-id,Values=${INSTANCE_ID}" --region "${REGION}" --query "Tags[?Key=='client' || Key=='subsystem'].Value" --output text)
    CLIENT=$(echo "${TAGS}" | awk '{print $1}')
    if [ -z "${CLIENT}" ]; then
        echo "error: cannot get CLIENT tag"
        exit 1
    fi
    SUBSYSTEM=$(echo "${TAGS}" | awk '{print $2}')
    if [ -z "${SUBSYSTEM}" ]; then
        echo "error: cannot get SUBSYSTEM tag"
        exit 1
    fi
    S3_BUCKET_URL="s3://data.${CLIENT}.${DOMAIN}/data/backup"
    DATE_PATH_FOR_LAST_MONTH_DB="${SUBSYSTEM}/${BACKUP_YEAR}/${BACKUP_MONTH}/${BACKUP_DAY}/${DAEMON}/${HOSTNAME}"
    DATE_PATH_FOR_CURRENT_DB="${SUBSYSTEM}/${PRESENT_YEAR}/${PRESENT_MONTH}/${PRESENT_DAY}/${DAEMON}/${HOSTNAME}"
    DOMAIN="nga911.com"
    COUCHDB_PORT="${COUCHDB_PORT:-5984}"
    COUCHDB_ALB_PORT="${COUCHDB_ALB_PORT:-15984}"
    COUCHDB_ADMINS_INI="${COUCHDB_ADMINS_INI:-/opt/couchdb/etc/local.d/admins.ini}"
    COUCHDB_USER=$(awk 'NR==2 {print $1}' "${COUCHDB_ADMINS_INI}")
    COUCHDB_PASS=$(awk 'NR==2 {print $3}' "${COUCHDB_ADMINS_INI}")
    COUCHDB_URL="http://${COUCHDB_USER}:${COUCHDB_PASS}@${HOSTNAME}:${COUCHDB_PORT}"
}

get_couchdb_alb_hostname() {
    NODES_COUNT=$(${AWS_CLI_BIN} ec2 describe-instances --region "${REGION}" --filters '[{"Name": "tag:subsystem", "Values": ["'"${SUBSYSTEM}"'"]},{"Name": "tag:db", "Values": ["true"]}]' --query 'length(Reservations[].Instances[])')
    if [ "${NODES_COUNT}" -eq 2 ]; then
        COUCHDB_ALB_URL="http://${COUCHDB_USER}:${COUCHDB_PASS}@${SUBSYSTEM}-db-0.${CLIENT}.${DOMAIN}:${COUCHDB_ALB_PORT}"
        export COUCHDB_ALB_URL
        echo "message: NGA AWS MAIN account couchdb host name is ${COUCHDB_ALB_URL}"
    else
        COUCHDB_ALB_URL="http://${COUCHDB_USER}:${COUCHDB_PASS}@${SUBSYSTEM}-db-1.${CLIENT}.${DOMAIN}:${COUCHDB_ALB_PORT}"
        export COUCHDB_ALB_URL
        echo "message: NGA AWS BACKUP account couchdb host name is ${COUCHDB_ALB_URL}"
    fi
}

create_work_dirs() {
    if [[ ! -d "${BACKUP_DIR}" ]]; then
        mkdir -p "${BACKUP_DIR}"
    fi
    if [[ ! -d "${RESTORE_DIR}" ]]; then
        mkdir -p "${RESTORE_DIR}"
    fi
    if [[ ! -d "${WORK_DIR}" ]]; then
        mkdir -p "${WORK_DIR}"
    fi
}

clean_work_dirs() {
    if [[ -d "${BACKUP_DIR}" ]]; then
        rm -rf "${BACKUP_DIR}"
    fi
    if [[ -d "${RESTORE_DIR}" ]]; then
        rm -rf "${RESTORE_DIR}"
    fi
    if [[ -d "${WORK_DIR}" ]]; then
        rm -rf "${WORK_DIR}"
    fi
}

# not done
manage_volume() {
    ACTION=$1
    case "${ACTION}" in
    create)
        echo "message: create volume"
        VOLUME_ID=$(${AWS_CLI_BIN} ec2 create-volume --volume-type gp2 --size "${VOLUME_SIZE}" --region "${REGION}" --availability-zone "${ZONE}" | jq -r '.VolumeId | tostring')
        export VOLUME_ID
        echo "message: attach volume ${VOLUME_ID}"
        ${AWS_CLI_BIN} ec2 attach-volume --volume-id "${VOLUME_ID}" --device "${DEVICE}" --instance-id "${INSTANCE_ID}" --region "${REGION}"
        echo "message: format volume"
        # ${MKFS_BIN} ${VOLUME}
        echo "message: mount volume"
        mount" ${VOLUME}" "${RESTORE_DIR}"
        echo "done"
        ;;
    delete)
        echo "message: unmount volume"
        umount "${RESTORE_DIR}"
        echo "message: detach volume"
        ${AWS_CLI_BIN} ec2 detach-volume --volume-id "${VOLUME_ID}" --instance-id "${INSTANCE_ID}" --region "${REGION}"
        echo "message: delete volume"
        ${AWS_CLI_BIN} ec2 delete-volume --volume-id "${VOLUME_ID}" --region "${REGION}"
        echo "done"
        ;;
    *)
        echo "error: unknown action"
        exit 1
        ;;
    esac
}

list_dbs() {
    DB_LIST=$(${CURL_BIN} -s -S "${COUCHDB_URL}"/_all_dbs/ | jq -r .[] | sed --expression='s/\//%2F/g; s/\+/%2B/g')
    FILTERED_DB_LIST=$(echo "${DB_LIST}" | grep -v -P "${EXCLUDED_DBS}")
    LAST_MONTH_DB_LIST=$(echo "${DB_LIST}" | grep -P "${LAST_MONTH}")
    ACCOUNTS_DB_LIST=$(echo "${DB_LIST}" | grep -P "account%2F" | grep -v -P "-")
}

list_host_dbs() {
    ${CURL_BIN} -s -S "${COUCHDB_URL}"/_all_dbs/ | jq -r .[] | sed --expression='s/\//%2F/g; s/\+/%2B/g'
}

get_doc_list() {
    local db=$1
    mkdir -p "$WORK_DIR"/"$db"
    ${CURL_BIN} -s -S "${COUCHDB_URL}"/"$db"/_all_docs?descending=true | jq -r '.rows | map(.id) | .[]' >"${WORK_DIR}/${db}/${DOC_LIST}"
}

split_doc_list() {
    local db=$1
    rm -Rf "${WORK_DIR}/${db}/doc_ids"
    mkdir -p "${WORK_DIR}/${db}/doc_ids"
    set +e
    csplit "${WORK_DIR}/${db}/${DOC_LIST}" --silent --digits 7 --elide-empty-files --keep-files --prefix="${WORK_DIR}/${db}/doc_ids/" "${DOCS_AMOUNT}" \{*\} 2>/dev/null
    set -e
}

send_save_command() {
    local db=$1
    local filename=$2
    local backup_path=$3
    local file_id
    file_id=$(basename "${filename}")
    jq -s -R 'split("\n") | map(select(. != "") | {"id" : .}) | {"docs": .}' "${filename}" >"${WORK_DIR}/request_body.json"
    mkdir -p "${BACKUP_DIR}"/"${backup_path}"/"${db}"
    ${CURL_BIN} -s -X POST \
        -H "Content-Type: application/json" \
        -d @"${WORK_DIR}"/request_body.json \
        --output - \
        "${COUCHDB_URL}/${db}/_bulk_get?include_docs=true&attachments=true" | gzip >"${BACKUP_DIR}/${backup_path}/${db}/${file_id}.json.gz"
    ${AWS_CLI_BIN} s3 mv "${BACKUP_DIR}/${backup_path}/${db}/${file_id}.json.gz" "${S3_BUCKET_URL}/${backup_path}/${db}/${file_id}.json.gz" --quiet
}

save_db() {
    local db=$1
    local backup_path=$2
    for filename in "${WORK_DIR}/${db}/doc_ids/"*; do
        [ -e "${filename}" ] || continue
        echo "backup ${filename}..."
        send_save_command "${db}" "${filename}" "${backup_path}"
    done
}

backup_dbs() {
    local date_path=$1
    local db_list=$2
    for db in $db_list; do
        get_doc_list "$db"
        split_doc_list "$db"
        save_db "$db" "$date_path"
    done || exit 1
}

backup() {
    if [[ ${CURRENT_DAY_MONTH} = "${BACKUP_DAY_MONTH_NUMBER}" ]]; then
        echo "backup: monthly databases"
        backup_dbs "${DATE_PATH_FOR_LAST_MONTH_DB}" "${LAST_MONTH_DB_LIST}"
        clean_work_dirs
        echo "done"
    else
        echo "backup: monthly databases"
        echo "not now, go rest..."
    fi
    if [[ ${CURRENT_DAY_WEEK} = "${BACKUP_DAY_WEEK_NUMBER}" ]]; then
        echo "backup: weekly databases"
        backup_dbs "${DATE_PATH_FOR_CURRENT_DB}" "${FILTERED_DB_LIST}"
        backup_dbs "${DATE_PATH_FOR_CURRENT_DB}" "${ACCOUNTS_DB_LIST}"
        clean_work_dirs
        echo "done"
    else
        echo "backup: weekly databases"
        echo "not now, go rest..."
    fi

}

backup_all_dbs() {
    list_dbs
    backup_dbs "${DATE_PATH_FOR_CURRENT_DB}" "${DB_LIST}"
}

backup_single_db() {
    local db=$1
    backup_dbs "${DATE_PATH_FOR_CURRENT_DB}" "${db}"
}

copy_from_s3() {
    local db_path
    db_path=$1
    mkdir -p "${RESTORE_DIR}/${db_path}"
    ${AWS_CLI_BIN} s3 cp "${S3_BUCKET_URL}/${db_path}" "${RESTORE_DIR}/${db_path}" --recursive --quiet
    if [ -f "${RESTORE_DIR}/${db_path}/0000000.json.gz" ]; then
        echo "files copied"
    else
        echo "restore directory empty, check backup path"
        exit 2
    fi
}

create_db() {
    local db=$1
    DB_EXISTS_CHECK=$(${CURL_BIN} -s -o /dev/null -w "%{http_code}" --head "${COUCHDB_ALB_URL}/${db}")
    if [ "${DB_EXISTS_CHECK}" = "200" ]; then
        echo "message: database ${db} already exists"
    else
        echo "message: create database ${db}"
        ${CURL_BIN} -s -X PUT "${COUCHDB_ALB_URL}:${COUCHDB_ALB_PORT}/${db}"
    fi
}

send_restore_command() {
    restore_from=${RESTORE_DIR}/"${db_path}"
    export restore_from
    local db=$1
    local filename=$2
    file_id=$(basename "${filename}")
    export file_id
    local db_target_host=$3
    local tmp_json="${restore_from}"/tmp.json
    gzip -d <"${filename}" | jq '.results | map(.docs[].ok | del(._rev)) | {"docs": .}' >"${tmp_json}"
    mkdir -p "${restore_from}"/bak
    mkdir -p "${restore_from}"/log

    echo "post ${file_id} to ${db}"
    ${CURL_BIN} -s -T "${tmp_json}" \
        -X POST "${COUCHDB_ALB_URL}/${db}/_bulk_docs" \
        --output "${restore_from}/log/${file_id}" \
        -H 'Content-Type: application/json'
    mv -f "${filename}" "${restore_from}"/bak
}

restore_db() {
    local db_path=$1
    local db_target_host=$2
    local db=$3
    for filename in "${RESTORE_DIR}/${db_path}"/*.json.gz; do
        [ -e "${filename}" ] || continue
        send_restore_command "${db}" "${filename}" "${db_target_host}"
    done
}

make_request_to_db_index() {
    local db_target_host=$1
    local db_name=$2
    local design_name=$3
    local view_name=$4
    ${CURL_BIN} -s "${db_target_host}/${db_name}/_design/${design_name}/_view/${view_name}?reduce=false&skip=0&limit=1" \
        -H 'Accept: application/json' -H 'Content-Type: application/json' \
        --compressed \
        --insecure | jq '.error , .reason | select ( . != null )'
}

refresh_monthdbs_views() {
    local db_target_host=$1
    local database=$2
    make_request_to_db_index "${db_target_host}" "${database}" cdrs crossbar_listing
    make_request_to_db_index "${db_target_host}" "${database}" cdrs listing_by_owner
    make_request_to_db_index "${db_target_host}" "${database}" cdrs summarize_cdrs
    make_request_to_db_index "${db_target_host}" "${database}" interactions correlation_listing_by_id
    make_request_to_db_index "${db_target_host}" "${database}" interactions interaction_listing
    make_request_to_db_index "${db_target_host}" "${database}" interactions interaction_listing_by_id
    make_request_to_db_index "${db_target_host}" "${database}" interactions interaction_listing_by_owner
    make_request_to_db_index "${db_target_host}" "${database}" recordings crossbar_listing
    make_request_to_db_index "${db_target_host}" "${database}" recordings listing_by_user
}

walk_by_indexes() {
    local db_target_host=$1
    local database=$2
    refresh_monthdbs_views "${db_target_host}" "${database}"
}

usage() {
    echo ""
    echo "The tool only works on CouchDB nodes in an AWS environment."
    echo "Usage:"
    echo "couchdb_backup.sh [-d] [-h] [-l] [-b] [--all | --single | --sheduled] [-v] [--attach | --detach] [-r] [--database \
        -d <db_name >]"
    echo " # Enable debug mode (should be first)"
    echo "  -h                                # Show this help"
    echo "  -l                                # List databases on current host"
    echo "  -b                                # Backup [all|single|sheduled] <db_name>"
    echo "      --all                         # Backup all databases from host (requires -b)"
    echo "      --single                      # Backup single database from host (requires -b)"
    echo "      --sheduled                    # Backup based on schedule (requires -b, for startup from systemd service)"
    echo "  -v (not complete)                 # Create/Delete volume [attach|detach]"
    echo "      --attach                      # Create volume"
    echo "      --detach                      # Delete volume"
    echo "  -r                                # Restore database [database <db_name>] from s3 bucket"
    echo "      --database                    # Restore single database"
    echo ""
    echo "Examples:"
    echo "  couchdb_backup.sh -l              # List databases on current host"
    echo "  couchdb_backup.sh -b --all        # Backup all databases from host to s3 bucket"
    echo "  couchdb_backup.sh -b --single     # Backup single database from host to s3 bucket"
    echo "  couchdb_backup.sh -b --sheduled   # Backup based on schedule"
    echo "  couchdb_backup.sh -v --attach     # Create, format and attach ec2 volume from current region and zone"
    echo "  couchdb_backup.sh -v --detach     # Detach and delete ec2 volume"
    echo "  couchdb_backup.sh -r --database   # Restore database"
    echo "  couchdb_backup.sh -d -v --attach  # Create volume witch debug mode"

    exit 1
}

parse_flags() {
    # Initialize variables

    backup_flag=false
    restore_flag=false

    if [ $# -eq 0 ]; then
        echo "error: no arguments provided"
        echo "help: for help use -h"
        exit 1
    fi

    # Loop through the positional parameters
    while [[ $# -gt 0 ]]; do
        case $1 in
        -d)
            debug_on
            ;;
        -h)
            usage
            ;;
        -l)
            init_default_vars
            init_aws_vars
            list_host_dbs
            ;;
        -b)
            backup_flag=true
            shift
            ;;
        -r)
            restore_flag=true
            shift
            ;;

        *)
            # Handle other parameters or display an error message
            echo "Invalid parameter: $1"
            echo "help: for help use -h"
            exit 1
            ;;
        esac

        # Check for nested flags
        if $backup_flag; then
            if [[ $# -lt 1 ]]; then
                echo "Error: Missing action flag: [--all|--single|--sheduled]"
                exit 1
            fi
            case $1 in
            --all)
                init_default_vars
                init_aws_vars
                # Perform backup all databases task
                echo "Performing backup all databases..."
                backup_all_dbs
                ;;
            --single)
                # Perform backup single database task
                if [[ $# -lt 2 ]]; then
                    echo "Error: Missing database name flag"
                    exit 1
                fi
                init_default_vars
                init_aws_vars
                db_name=$2
                echo "Performing backup for database: $db_name"
                backup_single_db "${db_name}"
                shift
                ;;
            --sheduled)

                # Perform backup based on schedule task
                echo "Performing backup based on schedule..."
                init_default_vars
                init_aws_vars
                create_work_dirs
                list_dbs
                backup
                clean_work_dirs
                exit 0
                ;;
            *)
                # Handle other parameters or display an error message
                echo "Invalid parameter: $1"
                exit 1
                ;;
            esac
        fi

        if $restore_flag; then
            if [[ $# -lt 1 ]]; then
                echo "Error: Missing action flag: [--database]"
                exit 1
            fi
            case $1 in
            --database)
                # Init variables
                init_default_vars
                init_aws_vars
                get_couchdb_alb_hostname
                # Perform restore task
                local db_subsystem=${SUBSYSTEM}
                local db_date=""
                local db_source_host=""
                local db_target_host=${COUCHDB_ALB_URL}
                local db_daemon=${DAEMON}
                local db_name=""

                if [ "$#" -ge 2 ]; then
                    db_source_host=$2
                fi
                if [ "$#" -ge 3 ]; then
                    db_date=$3
                fi
                if [ "$#" -ge 4 ]; then
                    db_name=$4
                fi
                shift

                echo "Performing restore..."

                if [ "$#" -lt 3 ]; then
                    echo "error: db restoring parameters is missing"
                    echo "current result:"
                    echo "date: ${db_date}"
                    echo "hostname: ${db_source_host}"
                    echo "db name: ${db_name}"
                    exit 1
                fi
                shift

                if [ -z "${db_source_host}" ] || [ -z "${db_date}" ] || [ -z "${db_name}" ]; then
                    echo "error: something is wrong, check log bellow"
                    echo "current result:"
                    echo "hostname: ${db_source_host}"
                    echo "date: ${db_date}"
                    echo "db name: ${db_name}"
                else
                    create_work_dirs
                    echo "copying from s3 bucket: ${db_name}"
                    copy_from_s3 "${db_subsystem}/${db_date}/${db_daemon}/${db_source_host}/${db_name}"
                    echo "checking database if not exists"
                    create_db "${db_name}"
                    echo "restoring ${db_name} to ${db_target_host}"
                    restore_db "${db_subsystem}/${db_date}/${db_daemon}/${db_source_host}/${db_name}" "${db_target_host}" "${db_name}"
                    echo "indexing: ${db_name}"
                    walk_by_indexes "${db_target_host}" "${db_name}"
                    clean_work_dirs
                    echo "message: done"
                fi
                shift
                ;;
            *)
                # Handle other parameters or display an error message
                echo "Invalid parameter: $1"
                exit 1
                ;;
            esac
        fi
        # Shift the positional parameters
        shift
    done
}

parse_flags "$@"
