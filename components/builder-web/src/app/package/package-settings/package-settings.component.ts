// Copyright (c) 2016-2017 Chef Software Inc. and/or applicable contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { Component, Inject, OnInit, OnDestroy, ViewChild } from '@angular/core';
import { ActivatedRoute } from '@angular/router';
import { MatDialog, MatDialogRef } from '@angular/material';
import { Subscription } from 'rxjs/subscription';
import { GitHubApiClient } from '../../GitHubApiClient';
import { GitHubRepo } from '../../github/repo/shared/github-repo.model';
import { requireSignIn } from '../../util';
import { AppStore } from '../../app.store';
import { addNotification, addProject, updateProject, setProjectIntegrationSettings, deleteProject, fetchProject } from '../../actions/index';
import config from '../../config';

@Component({
  selector: 'hab-package-settings',
  template: require('./package-settings.component.html')
})
export class PackageSettingsComponent implements OnInit, OnDestroy {
  name: string;
  origin: string;

  private sub: Subscription;

  constructor(private route: ActivatedRoute, private store: AppStore, private disconnectDialog: MatDialog) {
    this.sub = this.route.parent.params.subscribe((params) => {
      this.origin = params['origin'];
      this.name = params['name'];
    });
  }

  ngOnInit() {
    requireSignIn(this);
  }

  ngOnDestroy() {
    if (this.sub) {
      this.sub.unsubscribe();
    }
  }

  get project() {
    const project = this.store.getState().projects.current;
    const exists = this.store.getState().projects.ui.current.exists;

    const isMember = !!this.store.getState().origins.mine.find((o) => {
      return o.name === this.origin;
    });

    if (isMember && exists) {
      return project;
    }
  }

  get integrations() {
    return this.store.getState().origins.currentIntegrations.docker;
  }

  get loading() {
    return this.store.getState().projects.ui.current.loading;
  }

  saved(project) {
    window.scroll(0, 0);
  }
}