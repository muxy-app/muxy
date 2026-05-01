package com.muxy.protocol.dto

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class VCSStatusDTO(
    val branch: String,
    val aheadCount: Int,
    val behindCount: Int,
    val hasUpstream: Boolean,
    val stagedFiles: List<GitFileDTO>,
    val changedFiles: List<GitFileDTO>,
    val defaultBranch: String? = null,
    val pullRequest: VCSPullRequestDTO? = null,
)

@Serializable
data class GitFileDTO(
    val path: String,
    val status: GitFileStatusDTO,
    val isUntracked: Boolean = false,
)

@Serializable
enum class GitFileStatusDTO {
    @SerialName("added") ADDED,
    @SerialName("modified") MODIFIED,
    @SerialName("deleted") DELETED,
    @SerialName("renamed") RENAMED,
    @SerialName("copied") COPIED,
    @SerialName("untracked") UNTRACKED,
    @SerialName("unmerged") UNMERGED,
}

@Serializable
data class VCSPullRequestDTO(
    val url: String,
    val number: Int,
    val state: String,
    val isDraft: Boolean,
    val baseBranch: String,
)

@Serializable
data class VCSBranchesDTO(
    val current: String,
    val locals: List<String>,
    val defaultBranch: String? = null,
)

@Serializable
data class VCSCreatePRResultDTO(
    val url: String,
    val number: Int,
)
