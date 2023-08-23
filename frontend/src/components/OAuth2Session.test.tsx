// Copyright 2023 The Matrix.org Foundation C.I.C.
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

import { create } from "react-test-renderer";
import { describe, expect, it, vi } from "vitest";

import { FragmentType } from "../gql/fragment-masking";

import DateTime from "./DateTime";
import OAuth2Session, { OAUTH2_SESSION_FRAGMENT } from "./OAuth2Session";

// Mock out datetime to avoid timezones/date formatting
vi.mock("./DateTime", () => {
  const MockDateTime: typeof DateTime = ({ datetime }) => (
    <code>{datetime.toString()}</code>
  );
  return { default: MockDateTime };
});

describe("<OAuth2Session />", () => {
  const defaultProps = {
    session: {
      id: "session-id",
      scope:
        "openid urn:matrix:org.matrix.msc2967.client:api:* urn:matrix:org.matrix.msc2967.client:device:abcd1234",
      createdAt: "2023-06-29T03:35:17.451292+00:00",
      client: {
        id: "test-id",
        clientId: "test-client-id",
        clientName: "Element",
        clientUri: "https://element.io",
      },
    } as FragmentType<typeof OAUTH2_SESSION_FRAGMENT>,
  };

  const finishedAt = "2023-06-29T03:35:19.451292+00:00";

  it("renders an active session", () => {
    const component = create(<OAuth2Session {...defaultProps} />);
    expect(component.toJSON()).toMatchSnapshot();
  });

  it("renders a finished session", () => {
    const finishedSession = {
      ...defaultProps.session,
      finishedAt,
    };
    const component = create(<OAuth2Session session={finishedSession} />);
    expect(component.toJSON()).toMatchSnapshot();
  });
});