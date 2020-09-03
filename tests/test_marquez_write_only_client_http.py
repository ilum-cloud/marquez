# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
import datetime
import unittest

from marquez_client.models import DatasetType, SourceType, JobType, RunState
from marquez_client import Clients

import uuid
import logging
import logging.config
import yaml
import mock
import os

from marquez_client.utils import Utils

_NAMESPACE = "my-namespace"
log = logging.getLogger(__name__)


class TestMarquezWriteOnlyClientHttp(unittest.TestCase):
    def setUp(self):
        log.debug("MarquezWriteOnlyClient.setup(): ")

        with open('tests/logConfig.yaml', 'rt') as file:
            yamlConfig = yaml.safe_load(file.read())
            logging.config.dictConfig(yamlConfig)
            log.info("loaded logConfig.yaml")

        os.environ['MARQUEZ_BACKEND'] = 'http'
        self.client_wo_http = Clients.new_write_only_client()
        log.info("created marquez_client_wo_http.")

    @mock.patch("marquez_client.http_backend.HttpBackend.put")
    def test_create_namespace(self, mock_put):
        owner_name = "me"
        description = "my namespace for testing."

        self.client_wo_http.create_namespace(
            _NAMESPACE, owner_name, description)

    @mock.patch("marquez_client.http_backend.HttpBackend.put")
    def test_create_dataset(self, mock_put):
        dataset_name = "my-dataset"
        description = "My dataset for testing."

        fields = [
            {
                "name": "flight_id",
                "type": "INTEGER",
                "description": "flight id"
            },
            {
                "name": "flight_name",
                "type": "VARCHAR",
                "description": "flight name"
            },
            {
                "name": "flight_date",
                "type": "TIMESTAMP",
                "description": "flight date"
            }
        ]

        self.client_wo_http.create_dataset(
            namespace_name=_NAMESPACE,
            dataset_name=dataset_name,
            dataset_type=DatasetType.DB_TABLE,
            run_id=str(uuid.uuid4()),
            physical_name=dataset_name,
            source_name='my-source',
            description=description,
            schema_location=None,
            fields=fields,
            tags=None
        )

    @mock.patch("marquez_client.http_backend.HttpBackend.put")
    def test_create_datasource(self, mock_put):
        source_name = "flight_schedules_db"
        source_type = SourceType.POSTGRESQL
        source_url = "jdbc:postgresql://localhost:5432/test?" \
                     "user=fred&password=secret&ssl=true"
        description = "PostgreSQL - flight schedules database"

        self.client_wo_http.create_source(
            source_name=source_name,
            source_type=source_type,
            connection_url=source_url,
            description=description)

    @mock.patch("marquez_client.http_backend.HttpBackend.put")
    def test_create_job(self, mock_put):
        job_name = "my-job"
        input_dataset = [
            {
                "namespace": "my-namespace",
                "name": "public.mytable"
            }
        ]
        output_dataset = {
            "namespace": "my-namespace",
            "name": "public.mytable"
        }

        location = "https://github.com/my-jobs/blob/" \
                   "07f3d2dfc8186cadae9146719e70294a4c7a8ee8"

        context = {
            "SQL": "SELECT * FROM public.mytable;"
        }

        self.client_wo_http.create_job(
            namespace_name=_NAMESPACE,
            job_name=job_name,
            job_type=JobType.BATCH,
            location=location,
            input_dataset=input_dataset,
            output_dataset=output_dataset,
            context=context
        )

    @mock.patch("marquez_client.http_backend.HttpBackend.post")
    def test_create_job_run(self, mock_post):
        run_id = str(uuid.uuid4())
        job_name = "my-job"
        run_args = {
            "email": "me@mycorp.com",
            "emailOnFailure": "true",
            "emailOnRetry": "true",
            "retries": "1"
        }

        self.client_wo_http.create_job_run(
            namespace_name=_NAMESPACE,
            job_name=job_name,
            run_id=run_id,
            nominal_start_time=None,
            nominal_end_time=None,
            run_args=run_args,
            mark_as_running=True
        )

    @mock.patch("marquez_client.http_backend.HttpBackend.post")
    def test_mark_job_run_as_start(self, mock_post):
        run_id = str(uuid.uuid4())
        action_at = Utils.utc_now()

        self.client_wo_http.mark_job_run_as_started(
            run_id=run_id, action_at=action_at)

    @mock.patch("marquez_client.http_backend.HttpBackend.post")
    def test_mark_job_run_as_completed(self, mock_post):
        run_id = str(uuid.uuid4())
        action_at = Utils.utc_now()

        self.client_wo_http.mark_job_run_as_completed(
            run_id=run_id, action_at=action_at)

    @mock.patch("marquez_client.http_backend.HttpBackend.post")
    def test_mark_job_run_as_failed(self, mock_post):
        run_id = str(uuid.uuid4())
        action_at = Utils.utc_now()

        self.client_wo_http.mark_job_run_as_failed(
            run_id=run_id, action_at=action_at)

    @mock.patch("marquez_client.http_backend.HttpBackend.post")
    def test_mark_job_run_as_aborted(self, mock_post):
        run_id = str(uuid.uuid4())
        action_at = Utils.utc_now()

        self.client_wo_http.mark_job_run_as_aborted(
            run_id=run_id, action_at=action_at)


if __name__ == '__main__':
    unittest.main()
