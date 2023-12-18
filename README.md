https://docs.google.com/document/d/16F04RMME-M3c5nG2FK8OijmIZYgys5whJZUxJmRd3AM/edit?usp=sharing

# Утилита резервного копирования

[Оригинал описания задачи](https://docs.google.com/document/d/16F04RMME-M3c5nG2FK8OijmIZYgys5whJZUxJmRd3AM/edit?usp=sharing)

## Общая информация

Необходимо написать уитилиту резервного попирования базы данных CouchDB котороая будет работать в облаке Amazon как Lambda

## Этапность работ

1. Написание функции которая будет работать на локальном компе сотрудника или на сервере CouchDB.
2. Перернос програмы для работы как Lambda в облако Amazon.
3. Реализация лоигрования работы функции в систему мониторинга на базе Grafana Loki + prometheus (как опция допустимо расмотреть OpenTelemetry).

## Требования к конфиг файлу утилиты

Утилита должна конфигурироваться через с помощью текстового yaml файла.

В файле должно задаваться:
- url к базе данных CouchDB;
- login;
- password
- массив регулярных выражений для выборки баз еженедельного копирования;
- массив регулярных выражений для выборки баз ежемесячного копирования;
- для каждого массива должен задаваться график запуска резерного копирования (еженедельно, ежемесячно).
- S3 bucket url
- AWS_TOKEN
- AWS_SECRET

Пример конфига файла утилиты:
```yaml
database
  url: "http://[2345::6]:5984"
  login: "backup"
  password: "secret"
task:
  weekly:
    cron: "Sat *-*-1..7 18:00:00"
    databases:
    - "account%2F[0-9a-f]{2}%2F[0-9a-f]{2}%2F[0-9a-f]{16}"
    - "system_config"
    - "media"
    delay: 600
  mouthly:
    cron: "Sat *-*-1..7 18:00:00"
    databases:
    - "account%2F[0-9a-f]{2}%2F[0-9a-f]{2}%2F[0-9a-f]{16}-[0-9]{6}"
    delay: 600
    chunk: 1000
    backup_only_previus: true
bucket: "s3://data.example.com/folder"
token: "XXXXXXXXX"
secret: "YYYYYYYYYY"
prefix: "backup/ippbx"
suffix: "couchdb"
loki: "http://syslog-west.example.com:3100/loki/api/v1/push"
```

Параметры AWS_TOKEN и AWS_SECRET используются только для запуска на локальном компьютере сотрудника и явялются опиональными параметрами.

При запуске на сервере в облаке Amazon данные параметры должные браться из облака Amazon из IAM роли сервера.

При запуске как Lambda в облаке Amazon данные параметры должные браться из облака Amazon из IAM роли Lambda.

Параметр `delay` в задании резерного копирования определяет задержку в секундах между копированием баз данных.

Параметр `chunk` определяет сколько документов может быть выгружено из баз за один запрос резервного копирования. Если парметр не указан, то выгружать нужно все документы за один запрос.

Параметр `backup_only_previus` определят копировать базы только за предыдущий месяц. Базы за предыдущий месяц имеют в конце своего названия цифры в форме "-годмесяц", например “-202311".

Параметр `loki` опеределят по какому адресу нужно отправлять логи работы скрипта. В случае использвания OpenTelementry название параметра можно изменить.

Использование OpenTelementry необходимо согласовать перед реализацией.

## Требования к реализации

Логика скрипта должна быть реализована следующим образом:

1. получить список баз данных;
2. отранжировать базы которые должны быть скопированы в это запуск скрипта;
3. получить список документов в первой базе;
4. разбить список документов на части согласно парметру "chunk";
5. выгрузить документы согласно списку (или его части) из базы данных;
6. сжать выгруженные документы;
7. поместить полученный архив в хранилище S3 соблюдением  структуры папок: `${prefix}/${year}/${mouth}/${day}/${suffix}/${database_name}/${chunk_id.json.gz}`
8. Удалить созданые файлы;
9. повторить шаги "5-8" для следующих chunk до тех пока не будут выгружены все документы из базы;
10. повторить шаги "3-9" для следующих баз данных для текущего запуска;

## Эталонная реализация

[Эталонная релизация имеется на bash](https://github.com/yurybikuzin/couchdb_backup/blob/main/src/sh/couchdb_backup.sh) 

Она включает в себя резервное копирование базы и востановление базы. 

При реализация скрипта на Rust при возникновении вопросов следует руководствоваться примером на bash. 

## Требования к переносу на Lambda

1. Скрипт должен устанавливаться в облако Amazon через [CloudFormation](https://aws.amazon.com/ru/cloudformation/): [Building Rust Lambda functions with Cargo Lambda](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/building-rust.html).
2. Права скрипта должны задаваться через [AWS::Lambda::Permission](https://docs.aws.amazon.com/AWSCloudFormation/latest/UserGuide/aws-resource-lambda-permission.html)
3. Конфиг файл скрипта должен передаваться через yaml файл CloudFormation.

## Требования к логированию

При запуске на компьютере сотрудника или на сервере в Amazon достаточно вывода в консоль или journald.

При запуске как Lambda необходимо логи отправлять на [Loki сервер](https://github.com/grafana/loki). 

Допускается использование [OpenTelemtry](https://opentelemetry.io/), если исполнитель имеет его опыт эксплуатации. Использование [OpenTelemtry](https://opentelemetry.io/) необходимо согласовать перед реализацией.

Допускается поначалу работа без логов при запуске как Lambda. В конце логи должны быть.

## Прочие требования

Скрипт должен компилироваться на операционных системах [Fedora 38](https://alt.fedoraproject.org/cloud/) (обязательно), [CentOS 8](https://aws.amazon.com/marketplace/pp/prodview-tlabscjpjaocy) (желательно).

Целевая система для запуска на сервере Amazon, aarch.

Программа должна работать в среде [IPv6 only](https://aws.amazon.com/ru/blogs/networking-and-content-delivery/introducing-ipv6-only-subnets-and-ec2-instances/) и [dualstack (IPv6/IPv4)](https://docs.aws.amazon.com/AmazonS3/latest/userguide/dual-stack-endpoints.html).


## Prerequisites

[Using the AWS SAM CLI](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/using-sam-cli.html)

For Ubuntu 22

### [Zig](https://github.com/ziglang/zig/wiki/Install-Zig-from-a-Package-Manager)

```
sudo snap install zig --classic --beta

```

### [Cargo lambda](https://www.cargo-lambda.info/guide/installation.html)

```
cargo install --locked cargo-lambda
```

## Building and testing Rust Lambda functions requires [installing Docker](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/install-docker.html). [Alternative](https://www.digitalocean.com/community/tutorials/how-to-install-and-use-docker-on-ubuntu-22-04)

### [Installing the AWS SAM CLI](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/install-sam-cli.html)

### [AWS SAM prerequisites](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/prerequisites.html)

[Step 2: Create an IAM user account](https://us-east-1.console.aws.amazon.com/iamv2/home?region=eu-central-1#/users)

[Step 3: Create an access key ID and secret access key](https://us-east-1.console.aws.amazon.com/iamv2/home?region=eu-central-1#/users/details/42/create-access-key)

[Step 4: Install the AWS CLI](https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html)

### [Installing the AWS SAM CLI]https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/install-sam-cli.html

```
export SAM_CLI_BETA_RUST_CARGO_LAMBDA=1
```

in `src/rust` create `samconfig.toml`:

```toml
[default.build.parameters]
beta_features = true
[default.sync.parameters]
beta_features = true
```

[Rust-Based AWS Lambda With AWS CDK Deployment](https://medium.com/techhappily/rust-based-aws-lambda-with-aws-cdk-deployment-14a9a8652d62)



```
Commands you can use next
=========================
[*] Create pipeline: cd sam-app-schedule && sam pipeline init --bootstrap
[*] Validate SAM template: cd sam-app-schedule && sam validate
[*] Test Function in the Cloud: cd sam-app-schedule && sam sync --stack-name {stack-name} --watch
```

[Workaround](https://stackoverflow.com/questions/50791354/running-aws-sam-projects-locally-get-error/52252629#52252629) for

```
$ sam local invoke
Error: Running AWS SAM projects locally requires Docker. Have you got it installed and running?
```

```
sam sync --stack-name rust --watch
```
