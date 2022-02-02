#![allow(dead_code)]

use std::borrow::Cow;

use ego_tree::{NodeId, Tree};

use html5ever::driver::{self, ParseOpts};
use html5ever::tendril::StrTendril;
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::{Attribute, ExpandedName, QualName};
use tendril::TendrilSink;

use crate::scraper::element::{Comment, Doctype, Element, Node, ProcessingInstruction, Text};

pub struct Html {
    pub errors: Vec<String>,
    pub quirks_mode: QuirksMode,
    pub tree: Tree<Node>,
}

impl Html {
    pub fn new() -> Self {
        Html {
            errors: Vec::new(),
            quirks_mode: QuirksMode::NoQuirks,
            tree: Tree::new(Node::Document),
        }
    }

    pub fn parse(html: &str) -> Self {
        let parser = driver::parse_document(Self::new(), ParseOpts::default());
        parser.one(html)
    }
}

/// Note: does not support the `<template>` element.
impl TreeSink for Html {
    // A reference to a DOM node.
    type Handle = NodeId;
    // Overall result of parsing
    type Output = Self;

    // Consume this sink and return the overall result of parsing
    fn finish(self) -> Self {
        self
    }

    // Signal a parse error.
    fn parse_error(&mut self, msg: Cow<'static, str>) {
        self.errors.push(msg.to_string());
    }

    // Set the document's quirks mode.
    fn set_quirks_mode(&mut self, mode: QuirksMode) {
        self.quirks_mode = mode;
    }

    // Get a handle to the Document node.
    fn get_document(&mut self) -> Self::Handle {
        self.tree.root().id()
    }

    // Do two handles refer to the same node?
    fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool {
        x == y
    }

    // What is the name of this element?
    //
    // Should never be called on a non-element node; feel free to panic!.
    fn elem_name(&self, target: &Self::Handle) -> ExpandedName {
        self.tree
            .get(*target)
            .unwrap()
            .value()
            .as_element()
            .unwrap()
            .name
            .expanded()
    }

    // Create an element.
    //
    // When creating a template element (name.ns.expanded() == expanded_name!(html "template")), an
    // associated document fragment called the "template contents" should also be created. Later
    // calls to self.get_template_contents() with that given element return it.
    fn create_element(
        &mut self,
        name: QualName,
        attrs: Vec<Attribute>,
        _flags: ElementFlags,
    ) -> Self::Handle {
        let mut node = self
            .tree
            .orphan(Node::Element(Element::new(name.clone(), attrs)));

        if name.expanded() == expanded_name!(html "template") {
            node.append(Node::Fragment);
        }
        node.id()
    }

    // Create a comment node.
    fn create_comment(&mut self, text: StrTendril) -> Self::Handle {
        self.tree
            .orphan(Node::Comment(Comment { comment: text }))
            .id()
    }

    // Append a DOCTYPE element to the Document node.
    fn append_doctype_to_document(
        &mut self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        let doctype = Doctype {
            name,
            public_id,
            system_id,
        };
        self.tree.root_mut().append(Node::Doctype(doctype));
    }

    // Append a node as the last child of the given node. If this would produce adjacent sibling
    // text nodes, it should concatenate the text instead.
    //
    // The child node will not already have a parent.
    fn append(&mut self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        let mut parent = self.tree.get_mut(*parent).unwrap();

        match child {
            NodeOrText::AppendNode(id) => {
                parent.append_id(id);
            }

            NodeOrText::AppendText(text) => {
                // Skip over whitespace nodes
                if text.trim().is_empty() {
                    return;
                }

                let can_concat = parent
                    .last_child()
                    .map_or(false, |mut n| n.value().is_text());

                if can_concat {
                    let mut last_child = parent.last_child().unwrap();
                    match *last_child.value() {
                        Node::Text(ref mut t) => t.text.push_tendril(&text),
                        _ => unreachable!(),
                    }
                } else {
                    parent.append(Node::Text(Text { text }));
                }
            }
        }
    }

    // Append a node as the sibling immediately before the given node. If that node has no parent,
    // do nothing and return Err(new_node).
    //
    // The tree builder promises that sibling is not a text node. However its old previous sibling,
    // which would become the new node's previous sibling, could be a text node. If the new node is
    // also a text node, the two should be merged, as in the behavior of append.
    //
    // NB: new_node may have an old parent, from which it should be removed.
    fn append_before_sibling(
        &mut self,
        sibling: &Self::Handle,
        new_node: NodeOrText<Self::Handle>,
    ) {
        if let NodeOrText::AppendNode(id) = new_node {
            self.tree.get_mut(id).unwrap().detach();
        }

        let mut sibling = self.tree.get_mut(*sibling).unwrap();
        if sibling.parent().is_some() {
            match new_node {
                NodeOrText::AppendNode(id) => {
                    sibling.insert_id_before(id);
                }

                NodeOrText::AppendText(text) => {
                    let can_concat = sibling
                        .prev_sibling()
                        .map_or(false, |mut n| n.value().is_text());

                    if can_concat {
                        let mut prev_sibling = sibling.prev_sibling().unwrap();
                        match *prev_sibling.value() {
                            Node::Text(ref mut t) => t.text.push_tendril(&text),
                            _ => unreachable!(),
                        }
                    } else {
                        sibling.insert_before(Node::Text(Text { text }));
                    }
                }
            }
        }
    }

    // Detach the given node from its parent.
    fn remove_from_parent(&mut self, target: &Self::Handle) {
        self.tree.get_mut(*target).unwrap().detach();
    }

    // Remove all the children from node and append them to new_parent.
    fn reparent_children(&mut self, node: &Self::Handle, new_parent: &Self::Handle) {
        self.tree
            .get_mut(*new_parent)
            .unwrap()
            .reparent_from_id_append(*node);
    }

    // Add each attribute to the given element, if no attribute with that name already exists. The
    // tree builder promises this will never be called with something else than an element.
    fn add_attrs_if_missing(&mut self, target: &Self::Handle, attrs: Vec<Attribute>) {
        let mut node = self.tree.get_mut(*target).unwrap();
        let element = match *node.value() {
            Node::Element(ref mut e) => e,
            _ => unreachable!(),
        };

        for attr in attrs {
            element.attrs.entry(attr.name).or_insert(attr.value);
        }
    }

    // Get a handle to a template's template contents.
    //
    // The tree builder promises this will never be called with something else than a template
    // element.
    fn get_template_contents(&mut self, target: &Self::Handle) -> Self::Handle {
        self.tree.get(*target).unwrap().first_child().unwrap().id()
    }

    // Mark a HTML <script> element as "already started".
    fn mark_script_already_started(&mut self, _node: &Self::Handle) {}

    // Create Processing Instruction.
    fn create_pi(&mut self, target: StrTendril, data: StrTendril) -> Self::Handle {
        self.tree
            .orphan(Node::ProcessingInstruction(ProcessingInstruction {
                target,
                data,
            }))
            .id()
    }

    fn append_based_on_parent_node(
        &mut self,
        element: &Self::Handle,
        prev_element: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        if self.tree.get(*element).unwrap().parent().is_some() {
            self.append_before_sibling(element, child)
        } else {
            self.append(prev_element, child)
        }
    }
}
