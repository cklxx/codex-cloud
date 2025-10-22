/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  experimental: {
    esmExternals: false
  },
  transpilePackages: [
    "antd",
    "@ant-design/icons",
    "rc-util",
    "rc-picker",
    "rc-table",
    "rc-dialog",
    "rc-menu",
    "rc-pagination",
    "rc-select",
    "rc-tree"
  ],
  webpack: (config) => {
    config.module.rules.push({
      test: /\.m?js$/,
      resolve: {
        fullySpecified: false
      }
    });

    config.resolve.extensionAlias = {
      ...(config.resolve.extensionAlias ?? {}),
      ".js": [".js", ".ts", ".tsx", ".mjs"],
      ".mjs": [".mjs", ".js"]
    };

    return config;
  }
};

export default nextConfig;
