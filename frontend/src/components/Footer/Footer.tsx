// Copyright 2023, 2024 The Matrix.org Foundation C.I.C.
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

import { Link } from "@vector-im/compound-web";
import { useTranslation } from "react-i18next";

import { FragmentType, graphql, useFragment } from "../../gql";

import styles from "./Footer.module.css";

export const FRAGMENT = graphql(/* GraphQL */ `
  fragment Footer_siteConfig on SiteConfig {
    id
    imprint
    tosUri
    policyUri
  }
`);

type Props = {
  siteConfig: FragmentType<typeof FRAGMENT>;
};

const Footer: React.FC<Props> = ({ siteConfig }) => {
  const data = useFragment(FRAGMENT, siteConfig);
  const { t } = useTranslation();
  return (
    <footer className={styles.legalFooter}>
      {(data.policyUri || data.tosUri) && (
        <nav>
          {data.policyUri && (
            <Link
              href={data.policyUri}
              title={t("branding.privacy_policy.alt", {
                defaultValue: "Link to the service privacy policy",
              })}
            >
              {t("branding.privacy_policy.link", {
                defaultValue: "Privacy policy",
              })}
            </Link>
          )}

          {data.policyUri && data.tosUri && (
            <div className={styles.separator} aria-hidden="true">
              •
            </div>
          )}

          {data.tosUri && (
            <Link
              href={data.tosUri}
              title={t("branding.terms_and_conditions.alt", {
                defaultValue: "Link to the service terms and conditions",
              })}
            >
              {t("branding.terms_and_conditions.link", {
                defaultValue: "Terms and conditions",
              })}
            </Link>
          )}
        </nav>
      )}

      {data.imprint && <p className={styles.imprint}>{data.imprint}</p>}
    </footer>
  );
};

export default Footer;
