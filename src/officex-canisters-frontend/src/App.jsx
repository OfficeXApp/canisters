// sandbox_frontend

import React, { useState, useEffect, useRef, useCallback } from "react";
import {
  Layout,
  Form,
  Input,
  Button,
  message,
  List,
  Typography,
  Breadcrumb,
  Spin,
  Modal,
  Popconfirm,
} from "antd";
import {
  FolderOutlined,
  FileOutlined,
  HomeOutlined,
  EditOutlined,
  DeleteOutlined,
} from "@ant-design/icons";
import { AuthClient } from "@dfinity/auth-client";
import { Actor, HttpAgent, AnonymousIdentity } from "@dfinity/agent";
import { idlFactory } from "./declarations/officex-canisters-backend/officex-canisters-backend.did.js";
const { Header, Content, Footer } = Layout;
const { Title } = Typography;

// Define your canister IDs
const BACKEND_CANISTER_ID = "bkyz2-fmaaa-aaaaa-qaaaq-cai";
const FACTORY_CANISTER_ID = "br5f7-7uaaa-aaaaa-qaaca-cai";

function App() {
  // const { alias, icpSlug } = useIdentity();
  const authRef = useRef(null);
  const backendRef = useRef(null);
  const [identity, setIdentity] = useState(null);
  const [principal, setPrincipal] = useState(null);
  const [currentPath, setCurrentPath] = useState("BrowserCache::");
  const [files, setFiles] = useState([]);
  const [folders, setFolders] = useState([]);
  const [loading, setLoading] = useState(true);
  const [form] = Form.useForm();

  const [renameModalVisible, setRenameModalVisible] = useState(false);
  const [itemToRename, setItemToRename] = useState(null);
  const [newName, setNewName] = useState("");

  const initializeActor = useCallback(async () => {
    try {
      const authClient = await AuthClient.create();
      const anonymousIdentity = new AnonymousIdentity();
      setIdentity(anonymousIdentity.getPrincipal());

      const host = "http://127.0.0.1:4943";
      const agent = new HttpAgent({ identity: anonymousIdentity, host });

      // When deploying to the IC mainnet, remove the following line:
      await agent.fetchRootKey();

      // Enable more verbose logging
      agent.fetchRootKey().catch((err) => {
        console.warn(
          "Unable to fetch root key. Check to ensure that your local replica is running"
        );
        console.error(err);
      });

      // When deploying to the IC mainnet, remove the following line:
      await agent.fetchRootKey();

      const actor = Actor.createActor(idlFactory, {
        agent,
        canisterId: BACKEND_CANISTER_ID,
      });

      backendRef.current = actor;
      setLoading(false);
      console.log("Actor initialized successfully");
      console.log("backend", backendRef.current);
      ping();
      fetchFilesAndFolders();
    } catch (error) {
      console.error("Error initializing actor:", error);
      message.error("Failed to initialize. Please try refreshing the page.");
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    initializeActor();
  }, [initializeActor]);

  const ping = useCallback(async () => {
    if (!backendRef.current) return;
    try {
      const result = await backendRef.current.ping();
      console.log("Ping result:", result);
    } catch (error) {
      console.error("Error pinging backend:", error);
    }
  }, []);

  const fetchFilesAndFolders = useCallback(async () => {
    if (!backendRef.current) return;

    setLoading(true);
    try {
      console.log("currentPath", currentPath);
      const result = await backendRef.current.fetch_files_at_folder_path({
        full_folder_path: currentPath || "BrowserCache::",
        limit: 1000,
        after: 0,
      });
      console.log("fetch_files_at_folder_path.result", result);
      setFiles(result.files);
      setFolders(result.folders);
      const snapshot = await backendRef.current.snapshot_hashtables();
      console.log("snapshot_hashtables.result", snapshot);
    } catch (error) {
      console.error("Error fetching files and folders:", error);
      message.error("Failed to fetch files and folders");
    } finally {
      setLoading(false);
    }
  }, [currentPath]);

  const handleCreateFile = async () => {
    try {
      const values = await form.validateFields();
      const filePath = `${currentPath}${values.name}`;

      const result = await backendRef.current.upsert_file_to_hash_tables(
        filePath,
        { BrowserCache: null }
      );

      console.log("File upserted successfully:", result);
      message.success("File created/updated successfully");
      fetchFilesAndFolders();
      form.resetFields();
    } catch (error) {
      console.error("Error upserting file:", error);
      message.error("Failed to create/update file");
    }
  };

  const handleCreateFolder = async () => {
    try {
      const values = await form.validateFields();
      const fullFolderPath = `${currentPath}${values.name}`;

      const result = await backendRef.current.create_folder(fullFolderPath, {
        BrowserCache: null,
      });

      if ("Ok" in result) {
        console.log("Folder created successfully:", result.Ok);
        message.success("Folder created successfully");
        fetchFilesAndFolders();
        form.resetFields();
      } else if ("Err" in result) {
        throw new Error(result.Err);
      }
    } catch (error) {
      console.error("Error creating folder:", error);
      message.error(`Failed to create folder: ${error.message}`);
    }
  };

  // Utility function to normalize paths
  const normalizePath = (path) => {
    // Split the path into storage location and the rest
    const [storage, ...parts] = path.split("::");

    // Join the parts, remove consecutive slashes, and ensure a trailing slash
    const normalizedParts = parts
      .join("::")
      .split("/")
      .filter(Boolean)
      .join("/");

    return `${storage}::${normalizedParts}/`;
  };

  const handleItemClick = (item, isFolder) => {
    if (isFolder) {
      const newPath = normalizePath(
        `${currentPath}${item.original_folder_name}`
      );
      setCurrentPath(newPath);
    } else {
      // Handle file click (e.g., open file, show details, etc.)
      console.log("File clicked:", item);
    }
  };

  const handleBreadcrumbClick = (path) => {
    setCurrentPath(normalizePath(path));
  };

  const renderBreadcrumb = () => {
    const paths = currentPath.split("/").filter((p) => p);
    return (
      <Breadcrumb>
        <Breadcrumb.Item
          href="#"
          onClick={() => handleBreadcrumbClick("BrowserCache::")}
        >
          <HomeOutlined />
        </Breadcrumb.Item>
        {paths.map((path, index) => {
          const fullPath = normalizePath(
            `BrowserCache::${paths.slice(0, index + 1).join("/")}`
          );
          return (
            <Breadcrumb.Item
              key={fullPath}
              href="#"
              onClick={() => handleBreadcrumbClick(fullPath)}
            >
              {path}
            </Breadcrumb.Item>
          );
        })}
      </Breadcrumb>
    );
  };

  const handleCurrentPathChange = (e) => {
    setCurrentPath(normalizePath(e.target.value));
  };

  const handleRename = (item, isFolder) => {
    setItemToRename({ ...item, isFolder });
    setNewName(isFolder ? item.original_folder_name : item.original_file_name);
    setRenameModalVisible(true);
  };

  const handleDelete = async (item, isFolder) => {
    try {
      if (isFolder) {
        await backendRef.current.delete_folder(item.id);
      } else {
        await backendRef.current.delete_file(item.id);
      }
      message.success(`${isFolder ? "Folder" : "File"} deleted successfully`);
      fetchFilesAndFolders();
    } catch (error) {
      console.error(`Error deleting ${isFolder ? "folder" : "file"}:`, error);
      message.error(`Failed to delete ${isFolder ? "folder" : "file"}`);
    }
  };

  const handleRenameSubmit = async () => {
    try {
      if (itemToRename.isFolder) {
        await backendRef.current.rename_folder(itemToRename.id, newName);
      } else {
        await backendRef.current.rename_file(itemToRename.id, newName);
      }
      message.success(
        `${itemToRename.isFolder ? "Folder" : "File"} renamed successfully`
      );
      setRenameModalVisible(false);
      fetchFilesAndFolders();
    } catch (error) {
      console.error(
        `Error renaming ${itemToRename.isFolder ? "folder" : "file"}:`,
        error
      );
      message.error(
        `Failed to rename ${itemToRename.isFolder ? "folder" : "file"}`
      );
    }
  };

  console.log("currentPath", currentPath);
  console.log("Folders", folders);
  console.log("Files", files);

  return (
    <Layout className="layout" style={{ minHeight: "100vh" }}>
      <Header>
        <Title level={3} style={{ color: "white", margin: 0 }}>
          File Manager
        </Title>
      </Header>
      <Button>Create Drive</Button>
      <Content style={{ padding: "0 50px" }}>
        <div style={{ background: "#fff", padding: 24, minHeight: 280 }}>
          {renderBreadcrumb()}
          <Form form={form} layout="inline" style={{ marginBottom: 16 }}>
            <Form.Item
              name="currentPath"
              initialValue={currentPath}
              style={{ width: "300px" }}
            >
              <Input
                placeholder="Current Path"
                value={currentPath}
                onChange={handleCurrentPathChange}
                onPressEnter={fetchFilesAndFolders}
              />
            </Form.Item>
            <Form.Item>
              <Button onClick={fetchFilesAndFolders}>Go</Button>
            </Form.Item>
            <Form.Item
              name="name"
              rules={[{ required: true, message: "Please input a name!" }]}
            >
              <Input placeholder="Name" />
            </Form.Item>
            <Form.Item>
              <Button type="primary" onClick={handleCreateFile}>
                Create File
              </Button>
            </Form.Item>
            <Form.Item>
              <Button onClick={handleCreateFolder}>Create Folder</Button>
            </Form.Item>
          </Form>
          <Button type="primary" onClick={ping}>
            Ping
          </Button>
          {loading ? (
            <div style={{ textAlign: "center", marginTop: 20 }}>
              <Spin size="large" />
            </div>
          ) : (
            <List
              itemLayout="horizontal"
              dataSource={[...folders, ...files]}
              renderItem={(item) => {
                const isFolder = "subfolder_uuids" in item;
                return (
                  <List.Item
                    actions={[
                      <Button
                        icon={<EditOutlined />}
                        onClick={() => handleRename(item, isFolder)}
                      >
                        Rename
                      </Button>,
                      <Popconfirm
                        title={`Are you sure you want to delete this ${
                          isFolder ? "folder" : "file"
                        }?`}
                        onConfirm={() => handleDelete(item, isFolder)}
                        okText="Yes"
                        cancelText="No"
                      >
                        <Button icon={<DeleteOutlined />} danger>
                          Delete
                        </Button>
                      </Popconfirm>,
                    ]}
                    onClick={() => handleItemClick(item, isFolder)}
                    style={{ cursor: "pointer" }}
                  >
                    <List.Item.Meta
                      avatar={isFolder ? <FolderOutlined /> : <FileOutlined />}
                      title={
                        item.original_folder_name || item.original_file_name
                      }
                      description={item.full_folder_path || item.full_file_path}
                    />
                  </List.Item>
                );
              }}
            />
          )}
        </div>
      </Content>
      <Footer style={{ textAlign: "center" }}>
        OfficeX File Manager ©2023
      </Footer>
      <Modal
        title={`Rename ${itemToRename?.isFolder ? "Folder" : "File"}`}
        open={renameModalVisible}
        onOk={handleRenameSubmit}
        onCancel={() => setRenameModalVisible(false)}
      >
        <Input
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          placeholder="Enter new name"
        />
      </Modal>
    </Layout>
  );
}

export default App;
