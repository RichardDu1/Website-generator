"""
SEOplant Backend — Pydantic request/response schemas.
"""
from datetime import datetime
from typing import Optional

from pydantic import BaseModel, EmailStr


# ── Auth ──

class UserRegister(BaseModel):
    email: str
    password: str
    display_name: str = ""


class UserLogin(BaseModel):
    email: str
    password: str


class UserResponse(BaseModel):
    id: str
    email: str
    display_name: str
    plan: str
    credits_remaining: int
    created_at: datetime

    model_config = {"from_attributes": True}


class TokenResponse(BaseModel):
    access_token: str
    token_type: str = "bearer"
    user: UserResponse


# ── Projects ──

class ProjectCreate(BaseModel):
    keyword: str
    site_type: str = "website"
    name: str = ""


class ProjectUpdate(BaseModel):
    name: Optional[str] = None
    status: Optional[str] = None
    target_url: Optional[str] = None


class ProjectResponse(BaseModel):
    id: str
    name: str
    keyword: str
    site_type: str
    status: str
    target_url: str
    credits_spent: int
    created_at: datetime
    updated_at: datetime

    model_config = {"from_attributes": True}


# ── SEO Pipeline ──

class SEOAnalyzeRequest(BaseModel):
    keyword: str
    site_type: str = "website"
    language: str = "en"


class SEOGenerateRequest(BaseModel):
    project_id: str
    keyword: str
    site_type: str = "website"
    site_name: str = "My Website"
    language: str = "en"


# ── Billing ──

class SubscribeRequest(BaseModel):
    price_id: str  # Stripe price ID


class CreditPurchaseRequest(BaseModel):
    amount: int  # Number of credits to buy


# ── Deploy ──

class DeployRequest(BaseModel):
    project_id: str
    target: str = "vps"        # vps / vercel / cloudflare
    domain: str = ""
    server_host: str = ""
    server_user: str = "root"
    server_port: int = 22


class DeployResponse(BaseModel):
    id: str
    project_id: str
    target: str
    domain: str
    status: str
    created_at: datetime

    model_config = {"from_attributes": True}
